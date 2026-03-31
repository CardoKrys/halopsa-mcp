use std::sync::Arc;
use std::time::Duration;

use hmcp_common::chunking::{self, ActionChunkInput, TicketMetadataInput};
use hmcp_common::db::SemanticDb;
use hmcp_common::halopsa::{HaloPSAClient, ServiceTokenManager, TicketFilter};
use hmcp_common::types::*;

use crate::embed::Embedder;
use crate::AppState;

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const BATCH_SIZE: i64 = 50;

/// Background worker that polls for embed jobs and processes them.
pub async fn run_job_worker(state: AppState) {
    let worker_id = format!("embedder-{}", uuid::Uuid::new_v4());
    let db = match &state.db {
        Some(db) => db.clone(),
        None => return,
    };

    let token_manager = ServiceTokenManager::new(
        &state.halo_url,
        &state.embed_client_id,
        &state.embed_client_secret,
        state.embed_tenant.as_deref(),
        reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap(),
    );

    // Recover any stuck jobs from a previous crash
    if let Ok(count) = db.recover_worker_jobs(&worker_id).await {
        if count > 0 {
            eprintln!("Job worker: recovered {count} stuck jobs");
        }
    }

    eprintln!("Job worker {worker_id}: polling for jobs");

    loop {
        // Expire stale jobs from any worker (stuck > 30 minutes)
        let _ = db.expire_stale_jobs(1800).await;

        // Try to claim a job
        match db.claim_next_job(&worker_id).await {
            Ok(Some(job)) => {
                eprintln!("Job worker: claimed job {} (scope: {})", job.id, job.scope);
                if let Err(e) = process_job(&job, &db, &token_manager, &state.embedder).await {
                    eprintln!("Job worker: job {} failed: {e}", job.id);
                    let _ = db.complete_job(job.id, Some(&e)).await;
                }
            }
            Ok(None) => {
                // No jobs, wait
            }
            Err(e) => {
                eprintln!("Job worker: claim error: {e}");
            }
        }

        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

async fn process_job(
    job: &EmbedJob,
    db: &Arc<dyn SemanticDb>,
    token_manager: &ServiceTokenManager,
    embedder: &Arc<dyn Embedder>,
) -> Result<(), String> {
    let client = token_manager.client().await?;

    match job.scope.as_str() {
        "all" => process_all_tickets(job, db, &client, embedder).await,
        s if s.starts_with("ticket:") => {
            let ticket_id: i64 = s
                .strip_prefix("ticket:")
                .unwrap()
                .parse()
                .map_err(|e| format!("Invalid ticket ID: {e}"))?;
            process_single_ticket(job, db, &client, embedder, ticket_id).await
        }
        other => Err(format!("Unknown job scope: {other}")),
    }
}

async fn process_all_tickets(
    job: &EmbedJob,
    db: &Arc<dyn SemanticDb>,
    client: &HaloPSAClient,
    embedder: &Arc<dyn Embedder>,
) -> Result<(), String> {
    let mut page = 1;
    let mut total_processed = 0i64;
    // First pass: count total
    let (_, total_tickets) = client
        .list_tickets(1, 1, &TicketFilter::default())
        .await?;
    db.update_job_progress(job.id, 0, total_tickets).await?;

    loop {
        let (tickets, _) = client
            .list_tickets(page, BATCH_SIZE, &TicketFilter::default())
            .await?;

        if tickets.is_empty() {
            break;
        }

        for ticket_value in &tickets {
            let ticket_id = ticket_value
                .get("id")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            if ticket_id == 0 {
                continue;
            }

            if let Err(e) = embed_ticket(db, client, embedder, ticket_id).await {
                eprintln!("Failed to embed ticket {ticket_id}: {e}");
            }

            total_processed += 1;
            if total_processed % 10 == 0 {
                db.update_job_progress(job.id, total_processed, total_tickets)
                    .await?;
            }
        }

        page += 1;
    }

    db.update_job_progress(job.id, total_processed, total_tickets)
        .await?;
    db.complete_job(job.id, None).await?;
    eprintln!("Job {}: completed ({total_processed} tickets)", job.id);
    Ok(())
}

async fn process_single_ticket(
    job: &EmbedJob,
    db: &Arc<dyn SemanticDb>,
    client: &HaloPSAClient,
    embedder: &Arc<dyn Embedder>,
    ticket_id: i64,
) -> Result<(), String> {
    db.update_job_progress(job.id, 0, 1).await?;
    embed_ticket(db, client, embedder, ticket_id).await?;
    db.update_job_progress(job.id, 1, 1).await?;
    db.complete_job(job.id, None).await?;
    Ok(())
}

/// Embed a single ticket: fetch full details + actions, chunk, embed, store.
async fn embed_ticket(
    db: &Arc<dyn SemanticDb>,
    client: &HaloPSAClient,
    embedder: &Arc<dyn Embedder>,
    ticket_id: i64,
) -> Result<(), String> {
    // Fetch ticket
    let ticket = client.get_ticket(ticket_id).await?;

    let summary = ticket
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let details = ticket
        .get("details")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Check if content has changed
    let content_for_hash = format!("{summary}\n{details}");
    let content_hash = chunking::sha256_hex(&content_for_hash);

    if let Ok(Some(existing_hash)) = db.get_ticket_content_hash(ticket_id).await {
        if existing_hash == content_hash {
            return Ok(()); // No change, skip
        }
    }

    // Fetch actions
    let actions_raw = client.list_actions(ticket_id, true).await.unwrap_or_default();
    let action_inputs: Vec<ActionChunkInput> = actions_raw
        .iter()
        .map(|a| ActionChunkInput {
            note: a
                .get("note")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            who: a.get("who").and_then(|v| v.as_str()).map(String::from),
            outcome: a
                .get("outcome")
                .and_then(|v| v.as_str())
                .map(String::from),
            date: a
                .get("actiondate")
                .and_then(|v| v.as_str())
                .map(String::from),
        })
        .collect();

    // Build metadata
    let metadata = TicketMetadataInput {
        client_name: ticket
            .get("client_name")
            .and_then(|v| v.as_str())
            .map(String::from),
        site_name: ticket
            .get("site_name")
            .and_then(|v| v.as_str())
            .map(String::from),
        agent_name: ticket
            .get("agent_name")
            .and_then(|v| v.as_str())
            .map(String::from),
        team: ticket
            .get("team")
            .and_then(|v| v.as_str())
            .map(String::from),
        status: ticket
            .get("status")
            .and_then(|v| v.as_str())
            .map(String::from),
        priority: ticket
            .get("priority")
            .and_then(|v| v.as_str())
            .map(String::from),
        category: ticket
            .get("category_1")
            .and_then(|v| v.as_str())
            .map(String::from),
        custom_fields: Vec::new(), // TODO: extract custom fields
        assets: Vec::new(),        // TODO: fetch assets
    };

    // Chunk
    let chunks = chunking::chunk_ticket(ticket_id, &summary, &details, &action_inputs, &metadata);

    if chunks.is_empty() {
        return Ok(());
    }

    // Embed all chunks
    let texts: Vec<String> = chunks.iter().map(|c| c.content.clone()).collect();
    let embeddings = embedder.embed(&texts).await?;

    if embeddings.len() != chunks.len() {
        return Err(format!(
            "Embedding count mismatch: {} chunks, {} embeddings",
            chunks.len(),
            embeddings.len()
        ));
    }

    // Build chunk inserts
    let chunk_inserts: Vec<ChunkInsert> = chunks
        .iter()
        .zip(embeddings.iter())
        .map(|(chunk, embedding)| ChunkInsert {
            content: chunk.content.clone(),
            content_hash: chunk.content_hash.clone(),
            embedding: embedding.clone(),
            heading_path: chunk.heading_path.clone(),
        })
        .collect();

    // Store
    let ticket_meta = TicketMeta {
        ticket_id,
        summary: summary.clone(),
        content_hash,
        client_name: metadata.client_name.clone(),
        agent_name: metadata.agent_name.clone(),
        status: metadata.status.clone(),
        team: metadata.team.clone(),
    };

    db.upsert_ticket(&ticket_meta).await?;
    db.insert_chunks(ticket_id, &chunk_inserts).await?;

    Ok(())
}
