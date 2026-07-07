use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::Client;
use serde_json::{json, Value};
use tokio::sync::RwLock;

use hmcp_common::db::SemanticDb;
use hmcp_common::halopsa::HaloPSAClient;

/// Permission cache entry.
#[allow(dead_code)]
struct PermCacheEntry {
    accessible: bool,
    checked_at: Instant,
}

#[allow(dead_code)]
const PERM_CACHE_TTL: Duration = Duration::from_secs(300); // 5 minutes
#[allow(dead_code)]
const VECTOR_WEIGHT: f32 = 0.8;
const SIMILARITY_THRESHOLD: f32 = 0.3;
const MAX_CONCURRENT_PERM_CHECKS: usize = 10;

#[allow(dead_code)]
pub struct SemanticState {
    db: Arc<dyn SemanticDb>,
    embedder_url: String,
    webhook_secret: String,
    http: Client,
    /// Per-user permission cache: (mcp_token, ticket_id) → accessible
    perm_cache: RwLock<HashMap<(String, i64), PermCacheEntry>>,
}

impl SemanticState {
    pub fn new(db: Arc<dyn SemanticDb>, embedder_url: String, webhook_secret: String) -> Self {
        Self {
            db,
            embedder_url,
            webhook_secret,
            http: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
            perm_cache: RwLock::new(HashMap::new()),
        }
    }

    #[allow(dead_code)]
    pub fn webhook_secret(&self) -> &str {
        &self.webhook_secret
    }

    /// Embed a query string using the embedder sidecar.
    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, String> {
        let resp = self
            .http
            .post(format!("{}/embed", self.embedder_url))
            .json(&json!({"texts": [text]}))
            .send()
            .await
            .map_err(|e| format!("Embedder request failed: {e}"))?;

        if !resp.status().is_success() {
            return Err("Embedder returned error".into());
        }

        let data: Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse embedder response: {e}"))?;

        let embeddings = data
            .get("embeddings")
            .and_then(|v| v.as_array())
            .ok_or("No embeddings in response")?;

        let first = embeddings
            .first()
            .and_then(|v| v.as_array())
            .ok_or("Empty embeddings")?;

        Ok(first
            .iter()
            .filter_map(|v| v.as_f64().map(|f| f as f32))
            .collect())
    }

    /// Check if a user can access a ticket (with caching).
    #[allow(dead_code)]
    async fn check_permission(
        &self,
        client: &HaloPSAClient,
        ticket_id: i64,
    ) -> bool {
        // We don't have the MCP token here to use as cache key,
        // so we skip caching for now and just check directly
        client.can_access_ticket(ticket_id).await
    }

    /// Semantic search: embed query → vector search → permission filter → return results.
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
        client: &HaloPSAClient,
    ) -> Result<Value, String> {
        // Get query embedding
        let query_embedding = self.embed_query(query).await?;

        // Vector search (fetch more than needed to account for permission filtering)
        let candidates = self
            .db
            .vector_search(&query_embedding, limit * 3, SIMILARITY_THRESHOLD)
            .await?;

        if candidates.is_empty() {
            return Ok(json!({
                "results": [],
                "total_candidates": 0,
                "message": "No matching tickets found"
            }));
        }

        // Deduplicate by ticket_id (keep highest-scoring chunk per ticket)
        let mut best_by_ticket: HashMap<i64, (i64, f32)> = HashMap::new();
        for hit in &candidates {
            let entry = best_by_ticket
                .entry(hit.ticket_id)
                .or_insert((hit.chunk_id, hit.score));
            if hit.score > entry.1 {
                *entry = (hit.chunk_id, hit.score);
            }
        }

        // Permission filter (check up to MAX_CONCURRENT_PERM_CHECKS at a time)
        let mut accessible_tickets: Vec<(i64, i64, f32)> = Vec::new(); // (ticket_id, chunk_id, score)
        let ticket_ids: Vec<(i64, i64, f32)> = best_by_ticket
            .into_iter()
            .map(|(tid, (cid, score))| (tid, cid, score))
            .collect();

        for chunk in ticket_ids.chunks(MAX_CONCURRENT_PERM_CHECKS) {
            let mut futures = Vec::new();
            for &(ticket_id, chunk_id, score) in chunk {
                let client_ref = client;
                futures.push(async move {
                    let accessible = client_ref.can_access_ticket(ticket_id).await;
                    (ticket_id, chunk_id, score, accessible)
                });
            }

            let results = futures::future::join_all(futures).await;
            for (ticket_id, chunk_id, score, accessible) in results {
                if accessible {
                    accessible_tickets.push((ticket_id, chunk_id, score));
                }
            }

            if accessible_tickets.len() >= limit {
                break;
            }
        }

        accessible_tickets.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        accessible_tickets.truncate(limit);

        // Fetch chunk details
        let chunk_ids: Vec<i64> = accessible_tickets.iter().map(|(_, cid, _)| *cid).collect();
        let chunk_details = self.db.get_chunk_details(&chunk_ids).await?;

        let results: Vec<Value> = accessible_tickets
            .iter()
            .map(|(ticket_id, chunk_id, score)| {
                let detail = chunk_details.iter().find(|d| d.chunk_id == *chunk_id);
                json!({
                    "ticket_id": ticket_id,
                    "score": score,
                    "heading": detail.map(|d| d.heading_path.as_str()).unwrap_or(""),
                    "excerpt": detail.map(|d| {
                        if d.content.len() > 500 {
                            format!("{}...", hmcp_common::util::truncate_str(&d.content, 500))
                        } else {
                            d.content.clone()
                        }
                    }).unwrap_or_default(),
                })
            })
            .collect();

        Ok(json!({
            "results": results,
            "total_candidates": candidates.len(),
            "returned": results.len(),
        }))
    }

    /// Get embedding status.
    pub async fn embedding_status(&self) -> Result<Value, String> {
        let stats = self.db.get_stats().await?;
        let latest_job = self.db.get_latest_job().await?;
        let jobs = self.db.list_jobs(5).await?;

        Ok(json!({
            "total_indexed_tickets": stats.total_indexed_tickets,
            "total_chunks": stats.total_chunks,
            "latest_job": latest_job.map(|j| json!({
                "id": j.id,
                "scope": j.scope,
                "status": j.status,
                "progress": format!("{}/{}", j.done_tickets, j.total_tickets),
                "error": j.error,
            })),
            "recent_jobs": jobs.iter().map(|j| json!({
                "id": j.id,
                "scope": j.scope,
                "status": j.status,
                "progress": format!("{}/{}", j.done_tickets, j.total_tickets),
            })).collect::<Vec<_>>(),
        }))
    }

    /// Queue an embedding job.
    pub async fn queue_embed_job(&self, scope: &str) -> Result<(i64, bool), String> {
        self.db.create_embed_job(scope).await
    }

    /// List recent jobs.
    #[allow(dead_code)]
    pub async fn list_jobs(&self, recent: usize) -> Result<Vec<hmcp_common::types::EmbedJob>, String> {
        self.db.list_jobs(recent).await
    }
}

// Bring in futures for join_all
mod futures {
    pub mod future {
        use std::future::Future;

        pub async fn join_all<F, T>(futures: Vec<F>) -> Vec<T>
        where
            F: Future<Output = T>,
        {
            let mut results = Vec::with_capacity(futures.len());
            for f in futures {
                results.push(f.await);
            }
            results
        }
    }
}
