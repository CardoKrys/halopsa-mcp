use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, AeadCore};
use async_trait::async_trait;
use base64::Engine;
use rusqlite::{Connection, params};
use sha2::Digest;
use zeroize::Zeroizing;

use hmcp_common::config::access_token_ttl;
use hmcp_common::db::{DbBackend, SemanticDb};
use hmcp_common::types::*;

const BASE64: base64::engine::general_purpose::GeneralPurpose =
    base64::engine::general_purpose::STANDARD;

pub struct SqliteDb {
    conn: Arc<Mutex<Connection>>,
    encryption_key: Zeroizing<[u8; 32]>,
}

impl SqliteDb {
    pub fn open(path: &Path, encryption_key: &str) -> Self {
        let conn = Connection::open(path).expect("Failed to open SQLite database");
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS access_tokens (
                 token TEXT PRIMARY KEY,
                 halo_access_token TEXT NOT NULL,
                 halo_refresh_token TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_tokens_created ON access_tokens(created_at);
             CREATE TABLE IF NOT EXISTS refresh_tokens (
                 token TEXT PRIMARY KEY,
                 halo_access_token TEXT NOT NULL,
                 halo_refresh_token TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_refresh_created ON refresh_tokens(created_at);",
        )
        .expect("Failed to initialize database schema");

        let hash = sha2::Sha256::digest(encryption_key.as_bytes());
        let mut key = Zeroizing::new([0u8; 32]);
        key.copy_from_slice(&hash);

        Self {
            conn: Arc::new(Mutex::new(conn)),
            encryption_key: key,
        }
    }

    fn hash_token(token: &str) -> String {
        let hash = sha2::Sha256::digest(token.as_bytes());
        format!("{hash:x}")
    }

    fn encrypt(&self, plaintext: &str) -> String {
        let cipher = Aes256Gcm::new((&*self.encryption_key).into());
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .expect("AES-GCM encryption failed");
        let mut combined = nonce.to_vec();
        combined.extend_from_slice(&ciphertext);
        BASE64.encode(&combined)
    }

    fn decrypt(&self, encoded: &str) -> Result<String, String> {
        let combined = BASE64
            .decode(encoded)
            .map_err(|e| format!("Base64 decode failed: {e}"))?;
        if combined.len() < 12 {
            return Err("Ciphertext too short".into());
        }
        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = aes_gcm::Nonce::from_slice(nonce_bytes);
        let cipher = Aes256Gcm::new((&*self.encryption_key).into());
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| format!("Decryption failed: {e}"))?;
        String::from_utf8(plaintext).map_err(|e| format!("UTF-8 decode failed: {e}"))
    }

    fn now_secs() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }

    fn cutoff_secs(ttl: Duration) -> i64 {
        Self::now_secs() - ttl.as_secs() as i64
    }
}

#[async_trait]
impl DbBackend for SqliteDb {
    async fn insert_access_token(
        &self,
        mcp_token: &str,
        halo_access_token: &str,
        halo_refresh_token: &str,
    ) -> Result<(), String> {
        let conn = self.conn.clone();
        let hash = Self::hash_token(mcp_token);
        let enc_at = self.encrypt(halo_access_token);
        let enc_rt = self.encrypt(halo_refresh_token);
        let now = Self::now_secs();

        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            let count: i64 = c
                .query_row("SELECT COUNT(*) FROM access_tokens", [], |r| r.get(0))
                .unwrap_or(0);
            if count >= 10_000 {
                return Err("Token limit reached (10000)".into());
            }
            c.execute(
                "INSERT OR REPLACE INTO access_tokens (token, halo_access_token, halo_refresh_token, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![hash, enc_at, enc_rt, now],
            )
            .map_err(|e| format!("Insert failed: {e}"))?;
            Ok(())
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn get_access_token(&self, mcp_token: &str) -> Result<Option<(String, String)>, String> {
        let conn = self.conn.clone();
        let hash = Self::hash_token(mcp_token);
        let key = self.encryption_key.clone();

        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            let mut stmt = c
                .prepare("SELECT halo_access_token, halo_refresh_token FROM access_tokens WHERE token = ?1")
                .map_err(|e| format!("Prepare failed: {e}"))?;
            let result = stmt
                .query_row(params![hash], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                });
            match result {
                Ok((enc_at, enc_rt)) => {
                    let db = SqliteDb {
                        conn: Arc::new(Mutex::new(Connection::open_in_memory().unwrap())),
                        encryption_key: key,
                    };
                    let at = db.decrypt(&enc_at)?;
                    let rt = db.decrypt(&enc_rt)?;
                    Ok(Some((at, rt)))
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(format!("Query failed: {e}")),
            }
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn update_halo_tokens(
        &self,
        mcp_token: &str,
        halo_access_token: &str,
        halo_refresh_token: &str,
    ) -> Result<(), String> {
        let conn = self.conn.clone();
        let hash = Self::hash_token(mcp_token);
        let enc_at = self.encrypt(halo_access_token);
        let enc_rt = self.encrypt(halo_refresh_token);

        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            c.execute(
                "UPDATE access_tokens SET halo_access_token = ?1, halo_refresh_token = ?2 WHERE token = ?3",
                params![enc_at, enc_rt, hash],
            )
            .map_err(|e| format!("Update failed: {e}"))?;
            Ok(())
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn cleanup_expired_tokens(&self) -> Result<(), String> {
        let conn = self.conn.clone();
        let cutoff = Self::cutoff_secs(Duration::from_secs(access_token_ttl()));
        let refresh_cutoff = Self::cutoff_secs(Duration::from_secs(7 * 86400)); // 7 days

        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            c.execute("DELETE FROM access_tokens WHERE created_at < ?1", params![cutoff])
                .map_err(|e| format!("Cleanup failed: {e}"))?;
            c.execute("DELETE FROM refresh_tokens WHERE created_at < ?1", params![refresh_cutoff])
                .map_err(|e| format!("Cleanup failed: {e}"))?;
            Ok(())
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn insert_refresh_token(
        &self,
        mcp_refresh_token: &str,
        halo_access_token: &str,
        halo_refresh_token: &str,
    ) -> Result<(), String> {
        let conn = self.conn.clone();
        let hash = Self::hash_token(mcp_refresh_token);
        let enc_at = self.encrypt(halo_access_token);
        let enc_rt = self.encrypt(halo_refresh_token);
        let now = Self::now_secs();

        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            c.execute(
                "INSERT OR REPLACE INTO refresh_tokens (token, halo_access_token, halo_refresh_token, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![hash, enc_at, enc_rt, now],
            )
            .map_err(|e| format!("Insert failed: {e}"))?;
            Ok(())
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn get_refresh_token(
        &self,
        mcp_refresh_token: &str,
    ) -> Result<Option<(String, String)>, String> {
        let conn = self.conn.clone();
        let hash = Self::hash_token(mcp_refresh_token);
        let key = self.encryption_key.clone();

        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            let result = c.query_row(
                "SELECT halo_access_token, halo_refresh_token FROM refresh_tokens WHERE token = ?1",
                params![hash],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            );
            match result {
                Ok((enc_at, enc_rt)) => {
                    let db = SqliteDb {
                        conn: Arc::new(Mutex::new(Connection::open_in_memory().unwrap())),
                        encryption_key: key,
                    };
                    let at = db.decrypt(&enc_at)?;
                    let rt = db.decrypt(&enc_rt)?;
                    Ok(Some((at, rt)))
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(format!("Query failed: {e}")),
            }
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn delete_refresh_token(&self, mcp_refresh_token: &str) -> Result<(), String> {
        let conn = self.conn.clone();
        let hash = Self::hash_token(mcp_refresh_token);

        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            c.execute("DELETE FROM refresh_tokens WHERE token = ?1", params![hash])
                .map_err(|e| format!("Delete failed: {e}"))?;
            Ok(())
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn backup(&self, path: &Path) -> Result<(), String> {
        let conn = self.conn.clone();
        let backup_path = path.to_path_buf();

        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            if let Some(parent) = backup_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            c.execute_batch(&format!(
                "VACUUM INTO '{}'",
                backup_path.display()
            ))
            .map_err(|e| format!("Backup failed: {e}"))
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }
}

#[async_trait]
impl SemanticDb for SqliteDb {
    async fn init_semantic_tables(&self) -> Result<(), String> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            c.execute_batch(
                "CREATE TABLE IF NOT EXISTS tickets (
                     ticket_id INTEGER PRIMARY KEY,
                     summary TEXT NOT NULL,
                     content_hash TEXT NOT NULL,
                     client_name TEXT,
                     agent_name TEXT,
                     status TEXT,
                     team TEXT,
                     updated_at INTEGER NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS chunks (
                     id INTEGER PRIMARY KEY AUTOINCREMENT,
                     ticket_id INTEGER NOT NULL REFERENCES tickets(ticket_id) ON DELETE CASCADE,
                     content TEXT NOT NULL,
                     content_hash TEXT NOT NULL,
                     heading_path TEXT NOT NULL,
                     embedding BLOB,
                     UNIQUE(ticket_id, content_hash)
                 );
                 CREATE INDEX IF NOT EXISTS idx_chunks_ticket ON chunks(ticket_id);
                 CREATE TABLE IF NOT EXISTS embed_jobs (
                     id INTEGER PRIMARY KEY AUTOINCREMENT,
                     scope TEXT NOT NULL,
                     status TEXT NOT NULL DEFAULT 'pending',
                     done_tickets INTEGER NOT NULL DEFAULT 0,
                     total_tickets INTEGER NOT NULL DEFAULT 0,
                     started_at INTEGER,
                     finished_at INTEGER,
                     error TEXT,
                     worker_id TEXT,
                     created_at INTEGER NOT NULL
                 );
                 CREATE INDEX IF NOT EXISTS idx_jobs_status ON embed_jobs(status);
                 CREATE TABLE IF NOT EXISTS metadata (
                     key TEXT PRIMARY KEY,
                     value TEXT NOT NULL
                 );",
            )
            .map_err(|e| format!("Failed to create semantic tables: {e}"))
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn upsert_ticket(&self, meta: &TicketMeta) -> Result<(), String> {
        let conn = self.conn.clone();
        let meta = meta.clone();
        let now = Self::now_secs();

        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            c.execute(
                "INSERT INTO tickets (ticket_id, summary, content_hash, client_name, agent_name, status, team, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(ticket_id) DO UPDATE SET
                     summary = excluded.summary,
                     content_hash = excluded.content_hash,
                     client_name = excluded.client_name,
                     agent_name = excluded.agent_name,
                     status = excluded.status,
                     team = excluded.team,
                     updated_at = excluded.updated_at",
                params![
                    meta.ticket_id,
                    meta.summary,
                    meta.content_hash,
                    meta.client_name,
                    meta.agent_name,
                    meta.status,
                    meta.team,
                    now,
                ],
            )
            .map_err(|e| format!("Upsert ticket failed: {e}"))?;
            Ok(())
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn delete_ticket(&self, ticket_id: i64) -> Result<(), String> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            c.execute("DELETE FROM tickets WHERE ticket_id = ?1", params![ticket_id])
                .map_err(|e| format!("Delete ticket failed: {e}"))?;
            Ok(())
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn get_ticket_content_hash(&self, ticket_id: i64) -> Result<Option<String>, String> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            match c.query_row(
                "SELECT content_hash FROM tickets WHERE ticket_id = ?1",
                params![ticket_id],
                |row| row.get(0),
            ) {
                Ok(hash) => Ok(Some(hash)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(format!("Query failed: {e}")),
            }
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn insert_chunks(&self, ticket_id: i64, chunks: &[ChunkInsert]) -> Result<(), String> {
        let conn = self.conn.clone();
        let chunks: Vec<ChunkInsert> = chunks.to_vec();

        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            // Delete existing chunks for this ticket
            c.execute("DELETE FROM chunks WHERE ticket_id = ?1", params![ticket_id])
                .map_err(|e| format!("Delete chunks failed: {e}"))?;

            let mut stmt = c
                .prepare(
                    "INSERT INTO chunks (ticket_id, content, content_hash, heading_path, embedding)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .map_err(|e| format!("Prepare failed: {e}"))?;

            for chunk in &chunks {
                let embedding_blob: Vec<u8> = chunk
                    .embedding
                    .iter()
                    .flat_map(|f| f.to_le_bytes())
                    .collect();
                stmt.execute(params![
                    ticket_id,
                    chunk.content,
                    chunk.content_hash,
                    chunk.heading_path,
                    embedding_blob,
                ])
                .map_err(|e| format!("Insert chunk failed: {e}"))?;
            }
            Ok(())
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn get_chunk_details(&self, chunk_ids: &[i64]) -> Result<Vec<ChunkDetail>, String> {
        let conn = self.conn.clone();
        let ids = chunk_ids.to_vec();

        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            let placeholders: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!(
                "SELECT id, ticket_id, content, heading_path FROM chunks WHERE id IN ({placeholders})"
            );
            let mut stmt = c.prepare(&sql).map_err(|e| format!("Prepare failed: {e}"))?;

            let params: Vec<Box<dyn rusqlite::types::ToSql>> =
                ids.iter().map(|id| Box::new(*id) as Box<dyn rusqlite::types::ToSql>).collect();

            let rows = stmt
                .query_map(rusqlite::params_from_iter(params), |row| {
                    Ok(ChunkDetail {
                        chunk_id: row.get(0)?,
                        ticket_id: row.get(1)?,
                        content: row.get(2)?,
                        heading_path: row.get(3)?,
                    })
                })
                .map_err(|e| format!("Query failed: {e}"))?;

            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Row read failed: {e}"))
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn create_embed_job(&self, scope: &str) -> Result<(i64, bool), String> {
        let conn = self.conn.clone();
        let scope = scope.to_string();
        let now = Self::now_secs();

        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            // Check for existing active job
            let existing: Option<i64> = c
                .query_row(
                    "SELECT id FROM embed_jobs WHERE scope = ?1 AND status IN ('pending', 'running')",
                    params![scope],
                    |row| row.get(0),
                )
                .ok();

            if let Some(id) = existing {
                return Ok((id, false));
            }

            c.execute(
                "INSERT INTO embed_jobs (scope, status, created_at) VALUES (?1, 'pending', ?2)",
                params![scope, now],
            )
            .map_err(|e| format!("Create job failed: {e}"))?;
            Ok((c.last_insert_rowid(), true))
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn claim_next_job(&self, worker_id: &str) -> Result<Option<EmbedJob>, String> {
        let conn = self.conn.clone();
        let worker = worker_id.to_string();
        let now = Self::now_secs();

        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            let job_id: Option<i64> = c
                .query_row(
                    "SELECT id FROM embed_jobs WHERE status = 'pending' ORDER BY id ASC LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .ok();

            let Some(id) = job_id else {
                return Ok(None);
            };

            c.execute(
                "UPDATE embed_jobs SET status = 'running', worker_id = ?1, started_at = ?2 WHERE id = ?3",
                params![worker, now, id],
            )
            .map_err(|e| format!("Claim failed: {e}"))?;

            let job = c
                .query_row(
                    "SELECT id, scope, status, done_tickets, total_tickets, started_at, finished_at, error, worker_id
                     FROM embed_jobs WHERE id = ?1",
                    params![id],
                    |row| {
                        Ok(EmbedJob {
                            id: row.get(0)?,
                            scope: row.get(1)?,
                            status: row.get(2)?,
                            done_tickets: row.get(3)?,
                            total_tickets: row.get(4)?,
                            started_at: row.get(5)?,
                            finished_at: row.get(6)?,
                            error: row.get(7)?,
                            worker_id: row.get(8)?,
                        })
                    },
                )
                .map_err(|e| format!("Read job failed: {e}"))?;

            Ok(Some(job))
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn expire_stale_jobs(&self, stale_secs: i64) -> Result<usize, String> {
        let conn = self.conn.clone();
        let cutoff = Self::now_secs() - stale_secs;

        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            let count = c
                .execute(
                    "UPDATE embed_jobs SET status = 'pending', worker_id = NULL WHERE status = 'running' AND started_at < ?1",
                    params![cutoff],
                )
                .map_err(|e| format!("Expire failed: {e}"))?;
            Ok(count)
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn recover_worker_jobs(&self, worker_id: &str) -> Result<usize, String> {
        let conn = self.conn.clone();
        let worker = worker_id.to_string();

        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            let count = c
                .execute(
                    "UPDATE embed_jobs SET status = 'pending', worker_id = NULL WHERE status = 'running' AND worker_id = ?1",
                    params![worker],
                )
                .map_err(|e| format!("Recover failed: {e}"))?;
            Ok(count)
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn update_job_progress(&self, job_id: i64, done: i64, total: i64) -> Result<(), String> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            c.execute(
                "UPDATE embed_jobs SET done_tickets = ?1, total_tickets = ?2 WHERE id = ?3",
                params![done, total, job_id],
            )
            .map_err(|e| format!("Update progress failed: {e}"))?;
            Ok(())
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn complete_job(&self, job_id: i64, error: Option<&str>) -> Result<(), String> {
        let conn = self.conn.clone();
        let status = if error.is_some() { "failed" } else { "completed" };
        let error = error.map(|e| e.to_string());
        let now = Self::now_secs();

        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            c.execute(
                "UPDATE embed_jobs SET status = ?1, finished_at = ?2, error = ?3 WHERE id = ?4",
                params![status, now, error, job_id],
            )
            .map_err(|e| format!("Complete failed: {e}"))?;
            Ok(())
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn get_latest_job(&self) -> Result<Option<EmbedJob>, String> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            let result = c.query_row(
                "SELECT id, scope, status, done_tickets, total_tickets, started_at, finished_at, error, worker_id
                 FROM embed_jobs ORDER BY id DESC LIMIT 1",
                [],
                |row| {
                    Ok(EmbedJob {
                        id: row.get(0)?,
                        scope: row.get(1)?,
                        status: row.get(2)?,
                        done_tickets: row.get(3)?,
                        total_tickets: row.get(4)?,
                        started_at: row.get(5)?,
                        finished_at: row.get(6)?,
                        error: row.get(7)?,
                        worker_id: row.get(8)?,
                    })
                },
            );
            match result {
                Ok(job) => Ok(Some(job)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(format!("Query failed: {e}")),
            }
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn get_stats(&self) -> Result<EmbedStats, String> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            let total_indexed_tickets: i64 = c
                .query_row("SELECT COUNT(*) FROM tickets", [], |r| r.get(0))
                .unwrap_or(0);
            let total_chunks: i64 = c
                .query_row("SELECT COUNT(*) FROM chunks WHERE embedding IS NOT NULL", [], |r| r.get(0))
                .unwrap_or(0);
            Ok(EmbedStats {
                total_indexed_tickets,
                total_chunks,
            })
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn list_jobs(&self, recent: usize) -> Result<Vec<EmbedJob>, String> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            let mut stmt = c
                .prepare(
                    "SELECT id, scope, status, done_tickets, total_tickets, started_at, finished_at, error, worker_id
                     FROM embed_jobs ORDER BY id DESC LIMIT ?1",
                )
                .map_err(|e| format!("Prepare failed: {e}"))?;
            let rows = stmt
                .query_map(params![recent as i64], |row| {
                    Ok(EmbedJob {
                        id: row.get(0)?,
                        scope: row.get(1)?,
                        status: row.get(2)?,
                        done_tickets: row.get(3)?,
                        total_tickets: row.get(4)?,
                        started_at: row.get(5)?,
                        finished_at: row.get(6)?,
                        error: row.get(7)?,
                        worker_id: row.get(8)?,
                    })
                })
                .map_err(|e| format!("Query failed: {e}"))?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Row read failed: {e}"))
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn vector_search(
        &self,
        query_embedding: &[f32],
        limit: usize,
        threshold: f32,
    ) -> Result<Vec<SearchHit>, String> {
        let conn = self.conn.clone();
        let query = query_embedding.to_vec();

        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            let mut stmt = c
                .prepare("SELECT id, ticket_id, embedding FROM chunks WHERE embedding IS NOT NULL")
                .map_err(|e| format!("Prepare failed: {e}"))?;

            let mut hits: Vec<SearchHit> = stmt
                .query_map([], |row| {
                    let chunk_id: i64 = row.get(0)?;
                    let ticket_id: i64 = row.get(1)?;
                    let blob: Vec<u8> = row.get(2)?;
                    Ok((chunk_id, ticket_id, blob))
                })
                .map_err(|e| format!("Query failed: {e}"))?
                .filter_map(|r| r.ok())
                .filter_map(|(chunk_id, ticket_id, blob)| {
                    let embedding: Vec<f32> = blob
                        .chunks_exact(4)
                        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                        .collect();
                    let score = cosine_similarity(&query, &embedding);
                    if score >= threshold {
                        Some(SearchHit {
                            chunk_id,
                            ticket_id,
                            score,
                        })
                    } else {
                        None
                    }
                })
                .collect();

            hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
            hits.truncate(limit);
            Ok(hits)
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn clear_all_embeddings(&self) -> Result<(), String> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            c.execute_batch("DELETE FROM chunks; DELETE FROM tickets;")
                .map_err(|e| format!("Clear failed: {e}"))
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn alter_embedding_dimension(&self, _dims: usize) -> Result<(), String> {
        // SQLite uses BLOB columns — dimensionless, no-op
        Ok(())
    }

    async fn get_meta(&self, key: &str) -> Result<Option<String>, String> {
        let conn = self.conn.clone();
        let key = key.to_string();
        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            match c.query_row(
                "SELECT value FROM metadata WHERE key = ?1",
                params![key],
                |row| row.get(0),
            ) {
                Ok(v) => Ok(Some(v)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(format!("Query failed: {e}")),
            }
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }

    async fn set_meta(&self, key: &str, value: &str) -> Result<(), String> {
        let conn = self.conn.clone();
        let key = key.to_string();
        let value = value.to_string();
        tokio::task::spawn_blocking(move || {
            let c = conn.lock().unwrap();
            c.execute(
                "INSERT INTO metadata (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map_err(|e| format!("Set meta failed: {e}"))?;
            Ok(())
        })
        .await
        .map_err(|e| format!("Task failed: {e}"))?
    }
}

/// Cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f64;
    let mut norm_a = 0.0f64;
    let mut norm_b = 0.0f64;
    for i in 0..a.len() {
        let ai = a[i] as f64;
        let bi = b[i] as f64;
        dot += ai * bi;
        norm_a += ai * ai;
        norm_b += bi * bi;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        (dot / denom) as f32
    }
}
