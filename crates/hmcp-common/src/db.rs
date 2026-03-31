use std::path::Path;

use async_trait::async_trait;

use crate::types::*;

/// Core database operations (auth tokens, backups).
#[async_trait]
pub trait DbBackend: Send + Sync + 'static {
    /// Store an MCP access token mapped to encrypted HaloPSA credentials.
    /// Credentials stored: halo_access_token, halo_refresh_token.
    async fn insert_access_token(
        &self,
        mcp_token: &str,
        halo_access_token: &str,
        halo_refresh_token: &str,
    ) -> Result<(), String>;

    /// Retrieve and decrypt HaloPSA tokens for an MCP access token.
    /// Returns (halo_access_token, halo_refresh_token).
    async fn get_access_token(&self, mcp_token: &str) -> Result<Option<(String, String)>, String>;

    /// Update the HaloPSA tokens for an existing MCP access token (after refresh).
    async fn update_halo_tokens(
        &self,
        mcp_token: &str,
        halo_access_token: &str,
        halo_refresh_token: &str,
    ) -> Result<(), String>;

    /// Delete expired access tokens and refresh tokens.
    async fn cleanup_expired_tokens(&self) -> Result<(), String>;

    /// Store an MCP refresh token mapped to encrypted HaloPSA credentials.
    async fn insert_refresh_token(
        &self,
        mcp_refresh_token: &str,
        halo_access_token: &str,
        halo_refresh_token: &str,
    ) -> Result<(), String>;

    /// Retrieve HaloPSA tokens for an MCP refresh token.
    async fn get_refresh_token(
        &self,
        mcp_refresh_token: &str,
    ) -> Result<Option<(String, String)>, String>;

    /// Delete an MCP refresh token (used during rotation).
    async fn delete_refresh_token(&self, mcp_refresh_token: &str) -> Result<(), String>;

    /// Create a database backup.
    async fn backup(&self, path: &Path) -> Result<(), String>;
}

/// Semantic search database operations for ticket embeddings.
#[async_trait]
pub trait SemanticDb: Send + Sync + 'static {
    /// Create semantic search tables if they don't exist.
    async fn init_semantic_tables(&self) -> Result<(), String>;

    // --- Tickets ---

    async fn upsert_ticket(&self, meta: &TicketMeta) -> Result<(), String>;
    async fn delete_ticket(&self, ticket_id: i64) -> Result<(), String>;
    async fn get_ticket_content_hash(&self, ticket_id: i64) -> Result<Option<String>, String>;

    // --- Chunks + embeddings ---

    async fn insert_chunks(&self, ticket_id: i64, chunks: &[ChunkInsert]) -> Result<(), String>;
    async fn get_chunk_details(&self, chunk_ids: &[i64]) -> Result<Vec<ChunkDetail>, String>;

    // --- Job queue ---

    async fn create_embed_job(&self, scope: &str) -> Result<(i64, bool), String>;
    async fn claim_next_job(&self, worker_id: &str) -> Result<Option<EmbedJob>, String>;
    async fn expire_stale_jobs(&self, stale_secs: i64) -> Result<usize, String>;
    async fn recover_worker_jobs(&self, worker_id: &str) -> Result<usize, String>;
    async fn update_job_progress(&self, job_id: i64, done: i64, total: i64) -> Result<(), String>;
    async fn complete_job(&self, job_id: i64, error: Option<&str>) -> Result<(), String>;
    async fn get_latest_job(&self) -> Result<Option<EmbedJob>, String>;
    async fn get_stats(&self) -> Result<EmbedStats, String>;
    async fn list_jobs(&self, recent: usize) -> Result<Vec<EmbedJob>, String>;

    // --- Vector search ---

    async fn vector_search(
        &self,
        query_embedding: &[f32],
        limit: usize,
        threshold: f32,
    ) -> Result<Vec<SearchHit>, String>;

    async fn clear_all_embeddings(&self) -> Result<(), String>;
    async fn alter_embedding_dimension(&self, dims: usize) -> Result<(), String>;

    // --- Metadata key-value store ---

    async fn get_meta(&self, key: &str) -> Result<Option<String>, String>;
    async fn set_meta(&self, key: &str, value: &str) -> Result<(), String>;
}
