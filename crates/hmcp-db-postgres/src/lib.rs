use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm};
use async_trait::async_trait;
use base64::Engine;
use pgvector::Vector;
use sha2::Digest;
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Row;
use zeroize::Zeroizing;

use hmcp_common::config::access_token_ttl;
use hmcp_common::db::{DbBackend, SemanticDb};
use hmcp_common::types::*;

const BASE64: base64::engine::general_purpose::GeneralPurpose =
    base64::engine::general_purpose::STANDARD;

const DEFAULT_EMBEDDING_DIM: i32 = 768;
const META_KEY_EMBEDDING_DIM: &str = "embedding_dim";

pub struct PostgresDb {
    pool: PgPool,
    encryption_key: Zeroizing<[u8; 32]>,
}

impl PostgresDb {
    /// Connect to Postgres and initialize the schema.
    pub async fn connect(database_url: &str, encryption_key: &str) -> Result<Self, String> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(Duration::from_secs(10))
            .connect(database_url)
            .await
            .map_err(|e| format!("Failed to connect to Postgres: {e}"))?;

        let hash = sha2::Sha256::digest(encryption_key.as_bytes());
        let mut key = Zeroizing::new([0u8; 32]);
        key.copy_from_slice(&hash);

        let db = Self {
            pool,
            encryption_key: key,
        };

        db.init_schema().await?;
        Ok(db)
    }

    async fn init_schema(&self) -> Result<(), String> {
        // pgvector extension + core auth tables. Always safe to re-run.
        sqlx::query(
            "CREATE EXTENSION IF NOT EXISTS vector;

             CREATE TABLE IF NOT EXISTS access_tokens (
                 token TEXT PRIMARY KEY,
                 halo_access_token TEXT NOT NULL,
                 halo_refresh_token TEXT NOT NULL,
                 created_at BIGINT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_tokens_created ON access_tokens(created_at);

             CREATE TABLE IF NOT EXISTS refresh_tokens (
                 token TEXT PRIMARY KEY,
                 halo_access_token TEXT NOT NULL,
                 halo_refresh_token TEXT NOT NULL,
                 created_at BIGINT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_refresh_created ON refresh_tokens(created_at);",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Core schema init failed: {e}"))?;
        Ok(())
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

    /// Fetch the current embedding dimension from metadata, creating the chunks
    /// table and index at the default dim if it doesn't yet exist.
    async fn ensure_chunks_table(&self, dim: i32) -> Result<(), String> {
        let table_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'chunks')",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("Check chunks table failed: {e}"))?;

        if !table_exists {
            let sql = format!(
                "CREATE TABLE chunks (
                     id BIGSERIAL PRIMARY KEY,
                     ticket_id BIGINT NOT NULL REFERENCES tickets(ticket_id) ON DELETE CASCADE,
                     content TEXT NOT NULL,
                     content_hash TEXT NOT NULL,
                     heading_path TEXT NOT NULL,
                     embedding vector({dim}),
                     UNIQUE(ticket_id, content_hash)
                 );
                 CREATE INDEX idx_chunks_ticket ON chunks(ticket_id);
                 CREATE INDEX idx_chunks_embedding ON chunks USING hnsw (embedding vector_cosine_ops);"
            );
            sqlx::query(&sql)
                .execute(&self.pool)
                .await
                .map_err(|e| format!("Chunks table create failed: {e}"))?;

            sqlx::query(
                "INSERT INTO metadata (key, value) VALUES ($1, $2)
                 ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
            )
            .bind(META_KEY_EMBEDDING_DIM)
            .bind(dim.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Set embedding_dim meta failed: {e}"))?;
        }

        Ok(())
    }
}

#[async_trait]
impl DbBackend for PostgresDb {
    async fn insert_access_token(
        &self,
        mcp_token: &str,
        halo_access_token: &str,
        halo_refresh_token: &str,
    ) -> Result<(), String> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM access_tokens")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| format!("Count failed: {e}"))?;
        if count >= 10_000 {
            return Err("Token limit reached (10000)".into());
        }

        let hash = Self::hash_token(mcp_token);
        let enc_at = self.encrypt(halo_access_token);
        let enc_rt = self.encrypt(halo_refresh_token);
        let now = Self::now_secs();

        sqlx::query(
            "INSERT INTO access_tokens (token, halo_access_token, halo_refresh_token, created_at)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (token) DO UPDATE SET
                 halo_access_token = EXCLUDED.halo_access_token,
                 halo_refresh_token = EXCLUDED.halo_refresh_token,
                 created_at = EXCLUDED.created_at",
        )
        .bind(hash)
        .bind(enc_at)
        .bind(enc_rt)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Insert failed: {e}"))?;
        Ok(())
    }

    async fn get_access_token(&self, mcp_token: &str) -> Result<Option<(String, String)>, String> {
        let hash = Self::hash_token(mcp_token);
        let row = sqlx::query(
            "SELECT halo_access_token, halo_refresh_token FROM access_tokens WHERE token = $1",
        )
        .bind(hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Query failed: {e}"))?;

        let Some(row) = row else { return Ok(None) };
        let enc_at: String = row
            .try_get(0)
            .map_err(|e| format!("Column read failed: {e}"))?;
        let enc_rt: String = row
            .try_get(1)
            .map_err(|e| format!("Column read failed: {e}"))?;
        let at = self.decrypt(&enc_at)?;
        let rt = self.decrypt(&enc_rt)?;
        Ok(Some((at, rt)))
    }

    async fn update_halo_tokens(
        &self,
        mcp_token: &str,
        halo_access_token: &str,
        halo_refresh_token: &str,
    ) -> Result<(), String> {
        let hash = Self::hash_token(mcp_token);
        let enc_at = self.encrypt(halo_access_token);
        let enc_rt = self.encrypt(halo_refresh_token);

        sqlx::query(
            "UPDATE access_tokens SET halo_access_token = $1, halo_refresh_token = $2 WHERE token = $3",
        )
        .bind(enc_at)
        .bind(enc_rt)
        .bind(hash)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Update failed: {e}"))?;
        Ok(())
    }

    async fn cleanup_expired_tokens(&self) -> Result<(), String> {
        let cutoff = Self::cutoff_secs(Duration::from_secs(access_token_ttl()));
        let refresh_cutoff = Self::cutoff_secs(Duration::from_secs(7 * 86400));

        sqlx::query("DELETE FROM access_tokens WHERE created_at < $1")
            .bind(cutoff)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Cleanup access tokens failed: {e}"))?;

        sqlx::query("DELETE FROM refresh_tokens WHERE created_at < $1")
            .bind(refresh_cutoff)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Cleanup refresh tokens failed: {e}"))?;
        Ok(())
    }

    async fn insert_refresh_token(
        &self,
        mcp_refresh_token: &str,
        halo_access_token: &str,
        halo_refresh_token: &str,
    ) -> Result<(), String> {
        let hash = Self::hash_token(mcp_refresh_token);
        let enc_at = self.encrypt(halo_access_token);
        let enc_rt = self.encrypt(halo_refresh_token);
        let now = Self::now_secs();

        sqlx::query(
            "INSERT INTO refresh_tokens (token, halo_access_token, halo_refresh_token, created_at)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (token) DO UPDATE SET
                 halo_access_token = EXCLUDED.halo_access_token,
                 halo_refresh_token = EXCLUDED.halo_refresh_token,
                 created_at = EXCLUDED.created_at",
        )
        .bind(hash)
        .bind(enc_at)
        .bind(enc_rt)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Insert refresh failed: {e}"))?;
        Ok(())
    }

    async fn get_refresh_token(
        &self,
        mcp_refresh_token: &str,
    ) -> Result<Option<(String, String)>, String> {
        let hash = Self::hash_token(mcp_refresh_token);
        let row = sqlx::query(
            "SELECT halo_access_token, halo_refresh_token FROM refresh_tokens WHERE token = $1",
        )
        .bind(hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Query failed: {e}"))?;

        let Some(row) = row else { return Ok(None) };
        let enc_at: String = row
            .try_get(0)
            .map_err(|e| format!("Column read failed: {e}"))?;
        let enc_rt: String = row
            .try_get(1)
            .map_err(|e| format!("Column read failed: {e}"))?;
        let at = self.decrypt(&enc_at)?;
        let rt = self.decrypt(&enc_rt)?;
        Ok(Some((at, rt)))
    }

    async fn delete_refresh_token(&self, mcp_refresh_token: &str) -> Result<(), String> {
        let hash = Self::hash_token(mcp_refresh_token);
        sqlx::query("DELETE FROM refresh_tokens WHERE token = $1")
            .bind(hash)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Delete failed: {e}"))?;
        Ok(())
    }

    async fn backup(&self, _path: &Path) -> Result<(), String> {
        Err("Postgres backend does not support in-process backup; use pg_dump or the Postgres physical backup tooling".into())
    }
}

#[async_trait]
impl SemanticDb for PostgresDb {
    async fn init_semantic_tables(&self) -> Result<(), String> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS tickets (
                 ticket_id BIGINT PRIMARY KEY,
                 summary TEXT NOT NULL,
                 content_hash TEXT NOT NULL,
                 client_name TEXT,
                 agent_name TEXT,
                 status TEXT,
                 team TEXT,
                 updated_at BIGINT NOT NULL
             );

             CREATE TABLE IF NOT EXISTS embed_jobs (
                 id BIGSERIAL PRIMARY KEY,
                 scope TEXT NOT NULL,
                 status TEXT NOT NULL DEFAULT 'pending',
                 done_tickets BIGINT NOT NULL DEFAULT 0,
                 total_tickets BIGINT NOT NULL DEFAULT 0,
                 started_at BIGINT,
                 finished_at BIGINT,
                 error TEXT,
                 worker_id TEXT,
                 created_at BIGINT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_jobs_status ON embed_jobs(status);

             CREATE TABLE IF NOT EXISTS metadata (
                 key TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Semantic tables init failed: {e}"))?;

        self.ensure_chunks_table(DEFAULT_EMBEDDING_DIM).await?;
        Ok(())
    }

    async fn upsert_ticket(&self, meta: &TicketMeta) -> Result<(), String> {
        let now = Self::now_secs();
        sqlx::query(
            "INSERT INTO tickets (ticket_id, summary, content_hash, client_name, agent_name, status, team, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (ticket_id) DO UPDATE SET
                 summary = EXCLUDED.summary,
                 content_hash = EXCLUDED.content_hash,
                 client_name = EXCLUDED.client_name,
                 agent_name = EXCLUDED.agent_name,
                 status = EXCLUDED.status,
                 team = EXCLUDED.team,
                 updated_at = EXCLUDED.updated_at",
        )
        .bind(meta.ticket_id)
        .bind(&meta.summary)
        .bind(&meta.content_hash)
        .bind(&meta.client_name)
        .bind(&meta.agent_name)
        .bind(&meta.status)
        .bind(&meta.team)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Upsert ticket failed: {e}"))?;
        Ok(())
    }

    async fn delete_ticket(&self, ticket_id: i64) -> Result<(), String> {
        sqlx::query("DELETE FROM tickets WHERE ticket_id = $1")
            .bind(ticket_id)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Delete ticket failed: {e}"))?;
        Ok(())
    }

    async fn get_ticket_content_hash(&self, ticket_id: i64) -> Result<Option<String>, String> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT content_hash FROM tickets WHERE ticket_id = $1")
                .bind(ticket_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| format!("Query failed: {e}"))?;
        Ok(row.map(|(hash,)| hash))
    }

    async fn insert_chunks(&self, ticket_id: i64, chunks: &[ChunkInsert]) -> Result<(), String> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| format!("Begin tx failed: {e}"))?;

        sqlx::query("DELETE FROM chunks WHERE ticket_id = $1")
            .bind(ticket_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Delete existing chunks failed: {e}"))?;

        for chunk in chunks {
            let vec = Vector::from(chunk.embedding.clone());
            sqlx::query(
                "INSERT INTO chunks (ticket_id, content, content_hash, heading_path, embedding)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(ticket_id)
            .bind(&chunk.content)
            .bind(&chunk.content_hash)
            .bind(&chunk.heading_path)
            .bind(vec)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Insert chunk failed: {e}"))?;
        }

        tx.commit()
            .await
            .map_err(|e| format!("Commit failed: {e}"))?;
        Ok(())
    }

    async fn get_chunk_details(&self, chunk_ids: &[i64]) -> Result<Vec<ChunkDetail>, String> {
        if chunk_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows: Vec<(i64, i64, String, String)> = sqlx::query_as(
            "SELECT id, ticket_id, content, heading_path FROM chunks WHERE id = ANY($1)",
        )
        .bind(chunk_ids)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Query failed: {e}"))?;

        Ok(rows
            .into_iter()
            .map(|(chunk_id, ticket_id, content, heading_path)| ChunkDetail {
                chunk_id,
                ticket_id,
                content,
                heading_path,
            })
            .collect())
    }

    async fn create_embed_job(&self, scope: &str) -> Result<(i64, bool), String> {
        let existing: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM embed_jobs WHERE scope = $1 AND status IN ('pending', 'running') LIMIT 1",
        )
        .bind(scope)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Check existing job failed: {e}"))?;

        if let Some((id,)) = existing {
            return Ok((id, false));
        }

        let now = Self::now_secs();
        let (id,): (i64,) = sqlx::query_as(
            "INSERT INTO embed_jobs (scope, status, created_at) VALUES ($1, 'pending', $2) RETURNING id",
        )
        .bind(scope)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("Create job failed: {e}"))?;
        Ok((id, true))
    }

    async fn claim_next_job(&self, worker_id: &str) -> Result<Option<EmbedJob>, String> {
        // Atomic claim using SKIP LOCKED to avoid worker contention.
        let now = Self::now_secs();
        let row: Option<(i64, String, String, i64, i64, Option<i64>, Option<i64>, Option<String>, Option<String>)> = sqlx::query_as(
            "UPDATE embed_jobs
             SET status = 'running', worker_id = $1, started_at = $2
             WHERE id = (
                 SELECT id FROM embed_jobs
                 WHERE status = 'pending'
                 ORDER BY id ASC
                 LIMIT 1
                 FOR UPDATE SKIP LOCKED
             )
             RETURNING id, scope, status, done_tickets, total_tickets, started_at, finished_at, error, worker_id",
        )
        .bind(worker_id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Claim job failed: {e}"))?;

        Ok(row.map(|r| EmbedJob {
            id: r.0,
            scope: r.1,
            status: r.2,
            done_tickets: r.3,
            total_tickets: r.4,
            started_at: r.5,
            finished_at: r.6,
            error: r.7,
            worker_id: r.8,
        }))
    }

    async fn expire_stale_jobs(&self, stale_secs: i64) -> Result<usize, String> {
        let cutoff = Self::now_secs() - stale_secs;
        let result = sqlx::query(
            "UPDATE embed_jobs SET status = 'pending', worker_id = NULL
             WHERE status = 'running' AND started_at < $1",
        )
        .bind(cutoff)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Expire stale failed: {e}"))?;
        Ok(result.rows_affected() as usize)
    }

    async fn recover_worker_jobs(&self, worker_id: &str) -> Result<usize, String> {
        let result = sqlx::query(
            "UPDATE embed_jobs SET status = 'pending', worker_id = NULL
             WHERE status = 'running' AND worker_id = $1",
        )
        .bind(worker_id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Recover worker failed: {e}"))?;
        Ok(result.rows_affected() as usize)
    }

    async fn update_job_progress(&self, job_id: i64, done: i64, total: i64) -> Result<(), String> {
        sqlx::query("UPDATE embed_jobs SET done_tickets = $1, total_tickets = $2 WHERE id = $3")
            .bind(done)
            .bind(total)
            .bind(job_id)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Update progress failed: {e}"))?;
        Ok(())
    }

    async fn complete_job(&self, job_id: i64, error: Option<&str>) -> Result<(), String> {
        let status = if error.is_some() { "failed" } else { "completed" };
        let now = Self::now_secs();
        sqlx::query(
            "UPDATE embed_jobs SET status = $1, finished_at = $2, error = $3 WHERE id = $4",
        )
        .bind(status)
        .bind(now)
        .bind(error)
        .bind(job_id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Complete job failed: {e}"))?;
        Ok(())
    }

    async fn get_latest_job(&self) -> Result<Option<EmbedJob>, String> {
        let row: Option<(i64, String, String, i64, i64, Option<i64>, Option<i64>, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT id, scope, status, done_tickets, total_tickets, started_at, finished_at, error, worker_id
             FROM embed_jobs ORDER BY id DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Query failed: {e}"))?;

        Ok(row.map(|r| EmbedJob {
            id: r.0,
            scope: r.1,
            status: r.2,
            done_tickets: r.3,
            total_tickets: r.4,
            started_at: r.5,
            finished_at: r.6,
            error: r.7,
            worker_id: r.8,
        }))
    }

    async fn get_stats(&self) -> Result<EmbedStats, String> {
        let total_indexed_tickets: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tickets")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);
        let total_chunks: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM chunks WHERE embedding IS NOT NULL")
                .fetch_one(&self.pool)
                .await
                .unwrap_or(0);
        Ok(EmbedStats {
            total_indexed_tickets,
            total_chunks,
        })
    }

    async fn list_jobs(&self, recent: usize) -> Result<Vec<EmbedJob>, String> {
        let rows: Vec<(i64, String, String, i64, i64, Option<i64>, Option<i64>, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT id, scope, status, done_tickets, total_tickets, started_at, finished_at, error, worker_id
             FROM embed_jobs ORDER BY id DESC LIMIT $1",
        )
        .bind(recent as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Query failed: {e}"))?;

        Ok(rows
            .into_iter()
            .map(|r| EmbedJob {
                id: r.0,
                scope: r.1,
                status: r.2,
                done_tickets: r.3,
                total_tickets: r.4,
                started_at: r.5,
                finished_at: r.6,
                error: r.7,
                worker_id: r.8,
            })
            .collect())
    }

    async fn vector_search(
        &self,
        query_embedding: &[f32],
        limit: usize,
        threshold: f32,
    ) -> Result<Vec<SearchHit>, String> {
        let vec = Vector::from(query_embedding.to_vec());
        // pgvector `<=>` is cosine distance (0 = identical, 2 = opposite).
        // score = 1 - distance ∈ [-1, 1] where 1 is a perfect match.
        let max_distance = 1.0_f32 - threshold;
        let rows: Vec<(i64, i64, f64)> = sqlx::query_as(
            "SELECT id, ticket_id, (1 - (embedding <=> $1))::float8 AS score
             FROM chunks
             WHERE embedding IS NOT NULL
               AND (embedding <=> $1) <= $2
             ORDER BY embedding <=> $1
             LIMIT $3",
        )
        .bind(&vec)
        .bind(max_distance as f64)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Vector search failed: {e}"))?;

        Ok(rows
            .into_iter()
            .map(|(chunk_id, ticket_id, score)| SearchHit {
                chunk_id,
                ticket_id,
                score: score as f32,
            })
            .collect())
    }

    async fn clear_all_embeddings(&self) -> Result<(), String> {
        // Truncate both in one statement so chunks' FK doesn't block.
        sqlx::query("TRUNCATE chunks, tickets RESTART IDENTITY CASCADE")
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Clear failed: {e}"))?;
        Ok(())
    }

    async fn alter_embedding_dimension(&self, dims: usize) -> Result<(), String> {
        let dims = dims as i32;
        // Check current dim; if unchanged, nothing to do.
        let current: Option<String> = sqlx::query_scalar(
            "SELECT value FROM metadata WHERE key = $1",
        )
        .bind(META_KEY_EMBEDDING_DIM)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Read dim meta failed: {e}"))?;

        if current.as_deref() == Some(dims.to_string().as_str()) {
            return Ok(());
        }

        // Truncate chunks so the type change is always valid, then alter + rebuild index.
        let sql = format!(
            "TRUNCATE chunks RESTART IDENTITY;
             DROP INDEX IF EXISTS idx_chunks_embedding;
             ALTER TABLE chunks ALTER COLUMN embedding TYPE vector({dims});
             CREATE INDEX idx_chunks_embedding ON chunks USING hnsw (embedding vector_cosine_ops);"
        );
        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Alter embedding dim failed: {e}"))?;

        sqlx::query(
            "INSERT INTO metadata (key, value) VALUES ($1, $2)
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
        )
        .bind(META_KEY_EMBEDDING_DIM)
        .bind(dims.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Write dim meta failed: {e}"))?;

        Ok(())
    }

    async fn get_meta(&self, key: &str) -> Result<Option<String>, String> {
        let row: Option<(String,)> = sqlx::query_as("SELECT value FROM metadata WHERE key = $1")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| format!("Query failed: {e}"))?;
        Ok(row.map(|(v,)| v))
    }

    async fn set_meta(&self, key: &str, value: &str) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO metadata (key, value) VALUES ($1, $2)
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Set meta failed: {e}"))?;
        Ok(())
    }
}
