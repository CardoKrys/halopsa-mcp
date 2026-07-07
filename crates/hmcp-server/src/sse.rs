use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderName, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use reqwest::Client;
use serde_json::Value;
use subtle::ConstantTimeEq;
use tokio::sync::{Mutex, RwLock, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use zeroize::Zeroize;

use hmcp_common::db::DbBackend;
use hmcp_common::halopsa::HaloPSAClient;

use crate::mcp;
use crate::oauth::{AuthCode, AUTH_CODE_TTL, PendingAuth, RegisteredClient};
use crate::semantic::SemanticState;

const MAX_SESSIONS_PER_TOKEN: usize = 5;
const MAX_TOTAL_SESSIONS: usize = 1000;
const SESSION_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_REQUESTS_PER_MINUTE: u32 = 100;
// Registered OAuth clients are kept far longer than sessions/auth codes —
// they represent a long-lived integration (e.g. a Claude connector), not a
// single login. A restart already wipes this in-memory map and forces every
// connected client to re-register (standard MCP client behavior on
// invalid_client), so this TTL is just hygiene against unbounded growth.
const REGISTERED_CLIENT_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);

#[derive(Clone)]
pub struct AppState {
    pub halo_url: String,
    pub halo_client_id: String,
    pub halo_client_secret: String,
    pub halo_tenant: Option<String>,
    pub http_client: Client,
    sessions: Arc<RwLock<HashMap<String, Session>>>,
    pub auth_codes: Arc<RwLock<HashMap<String, AuthCode>>>,
    pub pending_auths: Arc<RwLock<HashMap<String, PendingAuth>>>,
    pub registered_clients: Arc<RwLock<HashMap<String, RegisteredClient>>>,
    pub db: Arc<dyn DbBackend>,
    pub known_urls: Vec<String>,
    pub authorize_rate_limit: Arc<Mutex<RateLimit>>,
    pub register_rate_limit: Arc<Mutex<RateLimit>>,
    streamable_sessions: Arc<RwLock<HashMap<String, Instant>>>,
    pub semantic: Option<Arc<SemanticState>>,
}

pub(crate) struct RateLimit {
    count: u32,
    max: u32,
    window_start: Instant,
}

impl RateLimit {
    pub(crate) fn new(max: u32) -> Self {
        Self {
            count: 0,
            max,
            window_start: Instant::now(),
        }
    }

    pub(crate) fn check(&mut self) -> Result<(), ()> {
        if self.window_start.elapsed() > Duration::from_secs(60) {
            self.count = 0;
            self.window_start = Instant::now();
        }
        if self.count >= self.max {
            return Err(());
        }
        self.count += 1;
        Ok(())
    }
}

struct Session {
    tx: mpsc::Sender<Result<Event, Infallible>>,
    client: HaloPSAClient,
    mcp_token: String,
    created_at: Instant,
    rate_limit: Arc<Mutex<RateLimit>>,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.mcp_token.zeroize();
    }
}

impl AppState {
    pub fn new(
        halo_url: String,
        halo_client_id: String,
        halo_client_secret: String,
        halo_tenant: Option<String>,
        db: Arc<dyn DbBackend>,
        known_urls: Vec<String>,
        semantic: Option<Arc<SemanticState>>,
    ) -> Self {
        let http_client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .build()
            .expect("Failed to build HTTP client");
        Self {
            halo_url: halo_url.trim_end_matches('/').to_string(),
            halo_client_id,
            halo_client_secret,
            halo_tenant,
            http_client,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            auth_codes: Arc::new(RwLock::new(HashMap::new())),
            pending_auths: Arc::new(RwLock::new(HashMap::new())),
            registered_clients: Arc::new(RwLock::new(HashMap::new())),
            db,
            known_urls,
            authorize_rate_limit: Arc::new(Mutex::new(RateLimit::new(20))),
            register_rate_limit: Arc::new(Mutex::new(RateLimit::new(10))),
            streamable_sessions: Arc::new(RwLock::new(HashMap::new())),
            semantic,
        }
    }

    pub fn spawn_cleanup(&self) {
        let sessions = self.sessions.clone();
        let auth_codes = self.auth_codes.clone();
        let pending_auths = self.pending_auths.clone();
        let registered_clients = self.registered_clients.clone();
        let db = self.db.clone();
        let streamable_sessions = self.streamable_sessions.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                {
                    let mut sessions = sessions.write().await;
                    sessions.retain(|sid, s| {
                        let expired = s.tx.is_closed() || s.created_at.elapsed() > SESSION_TTL;
                        if expired {
                            eprintln!("Session {sid} cleaned up");
                        }
                        !expired
                    });
                }
                {
                    let mut codes = auth_codes.write().await;
                    codes.retain(|_, c| c.created_at.elapsed() < AUTH_CODE_TTL);
                }
                {
                    let mut pending = pending_auths.write().await;
                    pending.retain(|_, p| p.created_at.elapsed() < Duration::from_secs(600));
                }
                {
                    let mut clients = registered_clients.write().await;
                    clients.retain(|_, c| c.created_at.elapsed() < REGISTERED_CLIENT_TTL);
                }
                {
                    let mut ss = streamable_sessions.write().await;
                    ss.retain(|_, created| created.elapsed() < SESSION_TTL);
                }
                if let Err(e) = db.cleanup_expired_tokens().await {
                    eprintln!("Token cleanup error: {e}");
                }
            }
        });
    }
}

/// Resolve Bearer token from Authorization header to a HaloPSA client.
async fn resolve_credentials(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(HaloPSAClient, String), Response> {
    let auth = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or_else(|| {
            let base_url = derive_base_url(headers, &state.known_urls);
            let body = serde_json::json!({
                "error": "unauthorized",
                "error_description": "Bearer token required"
            });
            (
                StatusCode::UNAUTHORIZED,
                [(
                    header::WWW_AUTHENTICATE,
                    format!(
                        "Bearer resource_metadata=\"{base_url}/.well-known/oauth-protected-resource\""
                    ),
                )],
                Json(body),
            )
                .into_response()
        })?;

    let mcp_token = auth.to_string();

    // Look up HaloPSA tokens from DB
    let (halo_at, _halo_rt) = state
        .db
        .get_access_token(&mcp_token)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e})),
            )
                .into_response()
        })?
        .ok_or_else(|| {
            let base_url = derive_base_url(headers, &state.known_urls);
            (
                StatusCode::UNAUTHORIZED,
                [(
                    header::WWW_AUTHENTICATE,
                    format!(
                        "Bearer resource_metadata=\"{base_url}/.well-known/oauth-protected-resource\""
                    ),
                )],
                Json(serde_json::json!({"error": "invalid_token"})),
            )
                .into_response()
        })?;

    let client = HaloPSAClient::new(&state.halo_url, &halo_at, state.http_client.clone());
    Ok((client, mcp_token))
}

/// Classic SSE: GET /mcp/sse
pub async fn handle_sse(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Sse<axum::response::sse::KeepAliveStream<ReceiverStream<Result<Event, Infallible>>>>, Response> {
    let (client, mcp_token) = resolve_credentials(&state, &headers).await?;

    // Validate credentials against HaloPSA
    client.validate().await.map_err(|e| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": format!("HaloPSA validation failed: {e}")})),
        )
            .into_response()
    })?;

    // Check session limits
    {
        let sessions = state.sessions.read().await;
        if sessions.len() >= MAX_TOTAL_SESSIONS {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Too many sessions"})),
            )
                .into_response());
        }
        let user_count = sessions
            .values()
            .filter(|s| s.mcp_token == mcp_token)
            .count();
        if user_count >= MAX_SESSIONS_PER_TOKEN {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({"error": "Too many sessions for this token"})),
            )
                .into_response());
        }
    }

    let session_id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = mpsc::channel(64);

    // Send endpoint event
    let endpoint_url = format!("/mcp/messages/?sessionId={session_id}");
    let _ = tx
        .send(Ok(Event::default()
            .event("endpoint")
            .data(&endpoint_url)))
        .await;

    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(
            session_id.clone(),
            Session {
                tx,
                client,
                mcp_token,
                created_at: Instant::now(),
                rate_limit: Arc::new(Mutex::new(RateLimit::new(MAX_REQUESTS_PER_MINUTE))),
            },
        );
    }

    eprintln!("SSE session created: {session_id}");

    Ok(Sse::new(ReceiverStream::new(rx)).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    ))
}

/// Classic SSE: POST /mcp/messages/?sessionId=...
pub async fn handle_message(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
    Json(request): Json<Value>,
) -> impl IntoResponse {
    let session_id = match params.get("sessionId") {
        Some(id) => id.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "sessionId required"})),
            )
                .into_response()
        }
    };

    let sessions = state.sessions.read().await;
    let session = match sessions.get(&session_id) {
        Some(s) => s,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Session not found"})),
            )
                .into_response()
        }
    };

    // Rate limit
    if session.rate_limit.lock().await.check().is_err() {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({"error": "Rate limit exceeded"})),
        )
            .into_response();
    }

    // Process JSON-RPC request
    let response = mcp::handle_request(&request, &session.client, state.semantic.as_deref()).await;

    match response {
        Some(resp) => {
            let _ = session
                .tx
                .send(Ok(Event::default().event("message").data(resp.to_string())))
                .await;
        }
        None => {} // Notification, no response
    }

    StatusCode::ACCEPTED.into_response()
}

/// Streamable HTTP: POST /mcp/sse
pub async fn handle_streamable(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<Value>,
) -> impl IntoResponse {
    let (client, _mcp_token) = match resolve_credentials(&state, &headers).await {
        Ok(c) => c,
        Err(e) => return e,
    };

    let response = mcp::handle_request(&request, &client, state.semantic.as_deref()).await;

    match response {
        Some(resp) => {
            let mut headers = HeaderMap::new();
            // Track session if this was an initialize request
            if request.get("method").and_then(|m| m.as_str()) == Some("initialize") {
                let sid = uuid::Uuid::new_v4().to_string();
                headers.insert(
                    HeaderName::from_static("mcp-session-id"),
                    sid.parse().unwrap(),
                );
                state
                    .streamable_sessions
                    .write()
                    .await
                    .insert(sid, Instant::now());
            }
            (StatusCode::OK, headers, Json(resp)).into_response()
        }
        None => StatusCode::ACCEPTED.into_response(),
    }
}

pub fn derive_base_url(headers: &HeaderMap, known_urls: &[String]) -> String {
    if !known_urls.is_empty() {
        let incoming_host = headers
            .get("host")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        for url in known_urls {
            if let Some(host) = url
                .strip_prefix("https://")
                .or_else(|| url.strip_prefix("http://"))
            {
                let host = host.split('/').next().unwrap_or(host);
                if host == incoming_host {
                    return url.clone();
                }
            }
        }
        return known_urls[0].clone();
    }

    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .filter(|s| *s == "http" || *s == "https")
        .unwrap_or("https");
    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");
    format!("{scheme}://{host}")
}

#[allow(dead_code)]
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.as_bytes().ct_eq(b.as_bytes()).into()
}
