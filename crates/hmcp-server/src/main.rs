mod mcp;
mod oauth;
mod semantic;
mod sse;
mod tools;

use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderName, Method};
use axum::response::Json;
use axum::{Router, routing::get};
use serde_json::json;
use tower_http::cors::{AllowOrigin, CorsLayer};

use hmcp_common::db::{DbBackend, SemanticDb};

#[tokio::main]
async fn main() {
    eprintln!("HaloPSA MCP Server v{}", env!("CARGO_PKG_VERSION"));

    let halo_url = env::var("HMCP_HALO_URL").expect("HMCP_HALO_URL is required");
    let halo_client_id =
        env::var("HMCP_HALO_CLIENT_ID").expect("HMCP_HALO_CLIENT_ID is required");
    let halo_client_secret =
        env::var("HMCP_HALO_CLIENT_SECRET").expect("HMCP_HALO_CLIENT_SECRET is required");
    let halo_tenant = env::var("HMCP_HALO_TENANT").ok().filter(|s| !s.is_empty());

    let host = env::var("HMCP_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port: u16 = env::var("HMCP_PORT")
        .unwrap_or_else(|_| "8080".into())
        .parse()
        .expect("HMCP_PORT must be a valid port number");

    let encryption_key = env::var("HMCP_ENCRYPTION_KEY")
        .expect("HMCP_ENCRYPTION_KEY is required (32+ character key for AES-256-GCM)");
    if encryption_key.len() < 32 {
        panic!("HMCP_ENCRYPTION_KEY must be at least 32 characters");
    }
    eprintln!("Encryption: enabled (AES-256-GCM)");

    // Database
    let db: Arc<dyn DbBackend> = {
        let db_path = env::var("HMCP_DB_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/data/halopsa-mcp.db"));
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        eprintln!("Database: SQLite ({})", db_path.display());
        Arc::new(hmcp_db_sqlite::SqliteDb::open(&db_path, &encryption_key))
    };

    // Semantic DB (same SQLite instance)
    let semantic_db: Option<Arc<dyn SemanticDb>> = {
        let db_path = env::var("HMCP_DB_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/data/halopsa-mcp.db"));
        Some(Arc::new(hmcp_db_sqlite::SqliteDb::open(
            &db_path,
            &encryption_key,
        )))
    };

    // Known URLs
    let known_urls = {
        let mut urls: Vec<String> = Vec::new();
        if let Ok(domain) = env::var("HMCP_PUBLIC_DOMAIN") {
            let domain = domain.trim().trim_end_matches('/');
            if !domain.is_empty() {
                urls.push(format!("https://{domain}"));
            }
        }
        if !urls.is_empty() {
            eprintln!("Known URLs: {}", urls.join(", "));
        }
        urls
    };

    // Semantic search
    let semantic_enabled = env::var("HMCP_SEMANTIC_SEARCH")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false);

    let semantic = if semantic_enabled {
        let embedder_url = env::var("HMCP_EMBEDDER_URL")
            .unwrap_or_else(|_| "http://hmcp-embedder:8081".into());

        match &semantic_db {
            Some(sdb) => {
                if let Err(e) = sdb.init_semantic_tables().await {
                    eprintln!("Semantic: failed to initialize tables: {e}");
                    None
                } else {
                    eprintln!("Semantic: enabled (embedder_url={embedder_url})");
                    let webhook_secret = env::var("HMCP_WEBHOOK_SECRET")
                        .unwrap_or_else(|_| String::new());
                    Some(Arc::new(semantic::SemanticState::new(
                        sdb.clone(),
                        embedder_url,
                        webhook_secret,
                    )))
                }
            }
            None => None,
        }
    } else {
        eprintln!("Semantic: disabled");
        None
    };

    let state = sse::AppState::new(
        halo_url.clone(),
        halo_client_id,
        halo_client_secret,
        halo_tenant,
        db,
        known_urls,
        semantic,
    );
    state.spawn_cleanup();

    let app = Router::new()
        .route("/mcp/sse", get(sse::handle_sse).post(sse::handle_streamable))
        .route(
            "/mcp/messages/",
            axum::routing::post(sse::handle_message),
        )
        .route(
            "/.well-known/oauth-authorization-server",
            get(oauth::handle_metadata),
        )
        .route(
            "/.well-known/oauth-protected-resource",
            get(oauth::handle_resource_metadata),
        )
        .route(
            "/authorize",
            get(oauth::handle_authorize),
        )
        .route("/callback", get(oauth::handle_callback))
        .route("/token", axum::routing::post(oauth::handle_token))
        .route("/register", axum::routing::post(oauth::handle_register))
        .route("/health", get(|| async { Json(json!({"status": "ok"})) }))
        .layer(DefaultBodyLimit::max(1024 * 1024))
        .layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::any())
                .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
                .allow_headers([
                    HeaderName::from_static("authorization"),
                    HeaderName::from_static("content-type"),
                    HeaderName::from_static("accept"),
                    HeaderName::from_static("mcp-session-id"),
                    HeaderName::from_static("mcp-protocol-version"),
                    HeaderName::from_static("last-event-id"),
                ])
                .expose_headers([HeaderName::from_static("mcp-session-id")]),
        )
        .with_state(state);

    let addr: SocketAddr = format!("{host}:{port}").parse().unwrap();
    eprintln!("HaloPSA MCP server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
