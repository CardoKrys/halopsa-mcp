mod embed;
mod pipeline;

use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::State;
use axum::response::Json;
use axum::{Router, routing::post};
use serde::Deserialize;
use serde_json::json;

use hmcp_common::db::SemanticDb;
use hmcp_db_postgres::PostgresDb;

#[derive(Clone)]
struct AppState {
    embedder: Arc<dyn embed::Embedder>,
    db: Option<Arc<dyn SemanticDb>>,
    halo_url: String,
    embed_client_id: String,
    embed_client_secret: String,
    embed_tenant: Option<String>,
}

#[derive(Deserialize)]
struct EmbedRequest {
    texts: Vec<String>,
}

#[tokio::main]
async fn main() {
    eprintln!("HaloPSA MCP Embedder v{}", env!("CARGO_PKG_VERSION"));

    let provider = env::var("HMCP_EMBED_PROVIDER").unwrap_or_else(|_| "local".into());
    let model = env::var("HMCP_EMBED_MODEL")
        .unwrap_or_else(|_| "BAAI/bge-base-en-v1.5".into());

    let embedder: Arc<dyn embed::Embedder> = match provider.as_str() {
        "openai" => {
            let api_key = env::var("HMCP_EMBED_API_KEY").expect("HMCP_EMBED_API_KEY required for openai provider");
            let api_url = env::var("HMCP_EMBED_API_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1/embeddings".into());
            eprintln!("Embedder: OpenAI ({model} via {api_url})");
            Arc::new(embed::OpenAIEmbedder::new(&api_key, &api_url, &model))
        }
        "ollama" => {
            let url = env::var("HMCP_EMBED_OLLAMA_URL")
                .unwrap_or_else(|_| "http://localhost:11434".into());
            eprintln!("Embedder: Ollama ({model} via {url})");
            Arc::new(embed::OllamaEmbedder::new(&url, &model))
        }
        _ => {
            eprintln!("Embedder: Local fastembed ({model})");
            Arc::new(embed::LocalEmbedder::new(&model))
        }
    };

    // Database for job queue. The embedder never decrypts user tokens, but it
    // shares the PostgresDb struct which holds an encryption key field, so we
    // supply a throwaway key here.
    let encryption_key = env::var("HMCP_ENCRYPTION_KEY")
        .unwrap_or_else(|_| "embedder-only-key-not-used-for-tokens".to_string());
    let db: Option<Arc<dyn SemanticDb>> = match env::var("HMCP_DATABASE_URL") {
        Ok(url) => match PostgresDb::connect(&url, &encryption_key).await {
            Ok(pg) => {
                let pg = Arc::new(pg);
                if let Err(e) = pg.init_semantic_tables().await {
                    eprintln!("Failed to init semantic tables: {e}");
                    None
                } else {
                    Some(pg as Arc<dyn SemanticDb>)
                }
            }
            Err(e) => {
                eprintln!("Failed to connect to Postgres: {e}");
                None
            }
        },
        Err(_) => {
            eprintln!("HMCP_DATABASE_URL not set; running without DB (embed endpoint only)");
            None
        }
    };

    let halo_url = env::var("HMCP_HALO_URL").unwrap_or_default();
    let embed_client_id = env::var("HMCP_EMBED_CLIENT_ID").unwrap_or_default();
    let embed_client_secret = env::var("HMCP_EMBED_CLIENT_SECRET").unwrap_or_default();
    let embed_tenant = env::var("HMCP_EMBED_TENANT").ok().filter(|s| !s.is_empty());

    let state = AppState {
        embedder: embedder.clone(),
        db: db.clone(),
        halo_url,
        embed_client_id,
        embed_client_secret,
        embed_tenant,
    };

    // Start job worker if DB is available and service account is configured
    if let Some(ref _db) = db {
        if !state.embed_client_id.is_empty() {
            let worker_state = state.clone();
            tokio::spawn(async move {
                pipeline::run_job_worker(worker_state).await;
            });
            eprintln!("Job worker: started");
        } else {
            eprintln!("Job worker: disabled (no HMCP_EMBED_CLIENT_ID)");
        }
    }

    let host = env::var("HMCP_EMBED_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port: u16 = env::var("HMCP_EMBED_PORT")
        .unwrap_or_else(|_| "8081".into())
        .parse()
        .expect("HMCP_EMBED_PORT must be valid");

    let app = Router::new()
        .route("/embed", post(handle_embed))
        .route("/health", axum::routing::get(|| async { Json(json!({"status": "ok"})) }))
        .with_state(state);

    let addr: SocketAddr = format!("{host}:{port}").parse().unwrap();
    eprintln!("Embedder listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn handle_embed(
    State(state): State<AppState>,
    Json(req): Json<EmbedRequest>,
) -> Json<serde_json::Value> {
    match state.embedder.embed(&req.texts).await {
        Ok(embeddings) => Json(json!({ "embeddings": embeddings })),
        Err(e) => Json(json!({ "error": e })),
    }
}
