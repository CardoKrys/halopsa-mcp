use std::time::{Duration, Instant};

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Json;
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::Digest;
use zeroize::Zeroize;

use hmcp_common::config::access_token_ttl;

use crate::sse::{AppState, derive_base_url};

pub const AUTH_CODE_TTL: Duration = Duration::from_secs(300);
const BASE64URL: base64::engine::general_purpose::GeneralPurpose =
    base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// MCP auth code: issued after HaloPSA OAuth completes.
#[allow(dead_code)]
pub struct AuthCode {
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub redirect_uri: String,
    pub client_id: String,
    pub halo_access_token: Option<String>,
    pub halo_refresh_token: Option<String>,
    pub created_at: Instant,
}

impl Drop for AuthCode {
    fn drop(&mut self) {
        if let Some(ref mut t) = self.halo_access_token {
            t.zeroize();
        }
        if let Some(ref mut t) = self.halo_refresh_token {
            t.zeroize();
        }
    }
}

/// A client that has completed dynamic registration (POST /register).
/// /authorize validates its redirect_uri against this record — without it,
/// any caller could redirect a completed OAuth flow (carrying a real
/// HaloPSA-backed access token) to an arbitrary attacker-controlled URL.
pub struct RegisteredClient {
    pub redirect_uris: Vec<String>,
    pub created_at: Instant,
}

/// Pending OAuth authorization: tracks state between /authorize → HaloPSA → /callback.
pub struct PendingAuth {
    pub mcp_code_challenge: Option<String>,
    pub mcp_code_challenge_method: Option<String>,
    pub mcp_redirect_uri: String,
    pub mcp_client_id: String,
    pub mcp_state: Option<String>,
    /// Our own PKCE verifier for the HaloPSA leg
    pub halo_code_verifier: String,
    pub created_at: Instant,
}

#[derive(Deserialize)]
pub struct AuthorizeParams {
    response_type: String,
    client_id: String,
    redirect_uri: String,
    state: Option<String>,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
}

#[derive(Deserialize)]
pub struct CallbackParams {
    code: String,
    state: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct TokenForm {
    grant_type: String,
    code: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    code_verifier: Option<String>,
    redirect_uri: Option<String>,
    refresh_token: Option<String>,
}

impl Drop for TokenForm {
    fn drop(&mut self) {
        if let Some(ref mut s) = self.client_secret {
            s.zeroize();
        }
        if let Some(ref mut v) = self.code_verifier {
            v.zeroize();
        }
        if let Some(ref mut r) = self.refresh_token {
            r.zeroize();
        }
    }
}

/// OAuth metadata: GET /.well-known/oauth-authorization-server
pub async fn handle_metadata(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let base = derive_base_url(&headers, &state.known_urls);
    Json(json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/authorize"),
        "token_endpoint": format!("{base}/token"),
        "registration_endpoint": format!("{base}/register"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["client_secret_post", "none"],
    }))
}

/// Resource metadata: GET /.well-known/oauth-protected-resource
pub async fn handle_resource_metadata(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let base = derive_base_url(&headers, &state.known_urls);
    Json(json!({
        "resource": format!("{base}/mcp/sse"),
        "authorization_servers": [base],
        "bearer_methods_supported": ["header"],
    }))
}

/// GET /authorize — Redirect to HaloPSA's OAuth authorize endpoint.
pub async fn handle_authorize(
    State(state): State<AppState>,
    Query(params): Query<AuthorizeParams>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if state.authorize_rate_limit.lock().await.check().is_err() {
        return (StatusCode::TOO_MANY_REQUESTS, "Rate limited").into_response();
    }

    if params.response_type != "code" {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "unsupported_response_type"})),
        )
            .into_response();
    }

    // OAuth 2.1 requires PKCE on every authorization-code flow. Without
    // this, a crafted /authorize link with no code_challenge lets an
    // attacker skip PKCE verification entirely at /token.
    if params.code_challenge.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_request", "error_description": "code_challenge is required"})),
        )
            .into_response();
    }

    // The redirect_uri must exactly match one registered for this client_id
    // via POST /register. Without this check, anyone could pass an
    // arbitrary redirect_uri and have a completed OAuth flow — carrying a
    // real HaloPSA-backed access token — redirected to an attacker-controlled
    // URL instead of the legitimate client.
    {
        let clients = state.registered_clients.read().await;
        let Some(client) = clients.get(&params.client_id) else {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "invalid_client", "error_description": "Unknown client_id — call POST /register first"})),
            )
                .into_response();
        };
        if !client.redirect_uris.iter().any(|u| u == &params.redirect_uri) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "invalid_request", "error_description": "redirect_uri does not match a registered value for this client_id"})),
            )
                .into_response();
        }
    }

    let base_url = derive_base_url(&headers, &state.known_urls);

    // Generate our own PKCE verifier for the HaloPSA leg
    let halo_verifier = uuid::Uuid::new_v4().to_string() + &uuid::Uuid::new_v4().to_string();
    let halo_challenge = {
        let hash = sha2::Sha256::digest(halo_verifier.as_bytes());
        BASE64URL.encode(hash)
    };

    // Generate a state token to link the callback back to this request
    let our_state = uuid::Uuid::new_v4().to_string();

    // Store pending auth
    {
        let mut pending = state.pending_auths.write().await;
        pending.insert(
            our_state.clone(),
            PendingAuth {
                mcp_code_challenge: params.code_challenge,
                mcp_code_challenge_method: params.code_challenge_method,
                mcp_redirect_uri: params.redirect_uri,
                mcp_client_id: params.client_id,
                mcp_state: params.state,
                halo_code_verifier: halo_verifier,
                created_at: Instant::now(),
            },
        );
    }

    // Build HaloPSA authorize URL
    let callback_url = format!("{base_url}/callback");
    let mut halo_auth_url = format!(
        "{}/auth/authorize?response_type=code&client_id={}&redirect_uri={}&code_challenge={}&code_challenge_method=S256&scope=all&state={}",
        state.halo_url,
        urlencoding::encode(&state.halo_client_id),
        urlencoding::encode(&callback_url),
        urlencoding::encode(&halo_challenge),
        urlencoding::encode(&our_state),
    );

    if let Some(ref tenant) = state.halo_tenant {
        halo_auth_url.push_str(&format!("&tenant={}", urlencoding::encode(tenant)));
    }

    Redirect::temporary(&halo_auth_url).into_response()
}

/// GET /callback — HaloPSA redirects back here after user authenticates.
pub async fn handle_callback(
    State(state): State<AppState>,
    Query(params): Query<CallbackParams>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let base_url = derive_base_url(&headers, &state.known_urls);
    let callback_url = format!("{base_url}/callback");

    // Look up the pending auth by state
    let pending = {
        let mut pending_auths = state.pending_auths.write().await;
        pending_auths.remove(&params.state)
    };

    let Some(pending) = pending else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid or expired state"})),
        )
            .into_response();
    };

    // Exchange the HaloPSA auth code for tokens
    let mut token_params = vec![
        ("grant_type".to_string(), "authorization_code".to_string()),
        ("code".to_string(), params.code),
        ("redirect_uri".to_string(), callback_url),
        ("client_id".to_string(), state.halo_client_id.clone()),
        ("client_secret".to_string(), state.halo_client_secret.clone()),
        ("code_verifier".to_string(), pending.halo_code_verifier.clone()),
    ];
    if let Some(ref tenant) = state.halo_tenant {
        token_params.push(("tenant".to_string(), tenant.clone()));
    }

    let token_resp = state
        .http_client
        .post(format!("{}/auth/token", state.halo_url))
        .form(&token_params)
        .send()
        .await;

    let token_resp = match token_resp {
        Ok(r) => r,
        Err(e) => {
            eprintln!("HaloPSA token exchange failed: {e}");
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": format!("Token exchange failed: {e}")})),
            )
                .into_response();
        }
    };

    if !token_resp.status().is_success() {
        let body = token_resp.text().await.unwrap_or_default();
        let preview = if body.len() > 500 { &body[..500] } else { &body };
        eprintln!("HaloPSA token exchange error: {preview}");
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": "HaloPSA authentication failed"})),
        )
            .into_response();
    }

    let token_data: Value = match token_resp.json().await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": format!("Failed to parse token response: {e}")})),
            )
                .into_response();
        }
    };

    let halo_access_token = token_data
        .get("access_token")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let halo_refresh_token = token_data
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if halo_access_token.is_empty() {
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": "No access_token from HaloPSA"})),
        )
            .into_response();
    }

    // Generate MCP auth code
    let mcp_code = uuid::Uuid::new_v4().to_string();
    {
        let mut codes = state.auth_codes.write().await;
        codes.insert(
            mcp_code.clone(),
            AuthCode {
                code_challenge: pending.mcp_code_challenge,
                code_challenge_method: pending.mcp_code_challenge_method,
                redirect_uri: pending.mcp_redirect_uri.clone(),
                client_id: pending.mcp_client_id,
                halo_access_token: Some(halo_access_token),
                halo_refresh_token: Some(halo_refresh_token),
                created_at: Instant::now(),
            },
        );
    }

    // Redirect back to Claude's redirect_uri with the MCP auth code
    let mut redirect = format!(
        "{}?code={}",
        pending.mcp_redirect_uri,
        urlencoding::encode(&mcp_code),
    );
    if let Some(ref mcp_state) = pending.mcp_state {
        redirect.push_str(&format!("&state={}", urlencoding::encode(mcp_state)));
    }

    Redirect::temporary(&redirect).into_response()
}

/// POST /token — Exchange MCP auth code for MCP access token.
pub async fn handle_token(
    State(state): State<AppState>,
    axum::Form(mut form): axum::Form<TokenForm>,
) -> impl IntoResponse {
    match form.grant_type.as_str() {
        "authorization_code" => handle_authorization_code_grant(state, &mut form).await,
        "refresh_token" => handle_refresh_token_grant(state, &mut form).await,
        _ => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "unsupported_grant_type"})),
        )
            .into_response(),
    }
}

async fn handle_authorization_code_grant(
    state: AppState,
    form: &mut TokenForm,
) -> Response {
    let code = match &form.code {
        Some(c) => c.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "invalid_request", "error_description": "code required"})),
            )
                .into_response()
        }
    };

    // Look up auth code
    let auth_code = {
        let mut codes = state.auth_codes.write().await;
        codes.remove(&code)
    };

    let Some(auth_code) = auth_code else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_grant"})),
        )
            .into_response();
    };

    // Verify PKCE
    if let Some(ref challenge) = auth_code.code_challenge {
        let verifier = match &form.code_verifier {
            Some(v) => v,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "invalid_request", "error_description": "code_verifier required"})),
                )
                    .into_response()
            }
        };

        let method = auth_code
            .code_challenge_method
            .as_deref()
            .unwrap_or("S256");
        if method != "S256" {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "invalid_request", "error_description": "Only S256 supported"})),
            )
                .into_response();
        }

        let computed = {
            let hash = sha2::Sha256::digest(verifier.as_bytes());
            BASE64URL.encode(hash)
        };

        if computed != *challenge {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "invalid_grant", "error_description": "PKCE verification failed"})),
            )
                .into_response();
        }
    }

    let halo_at = auth_code.halo_access_token.as_deref().unwrap_or("");
    let halo_rt = auth_code.halo_refresh_token.as_deref().unwrap_or("");

    // Issue MCP tokens
    let mcp_access_token = uuid::Uuid::new_v4().to_string();
    let mcp_refresh_token = uuid::Uuid::new_v4().to_string();

    if let Err(e) = state
        .db
        .insert_access_token(&mcp_access_token, halo_at, halo_rt)
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Token storage failed: {e}")})),
        )
            .into_response();
    }

    if let Err(e) = state
        .db
        .insert_refresh_token(&mcp_refresh_token, halo_at, halo_rt)
        .await
    {
        eprintln!("Failed to store refresh token: {e}");
    }

    Json(json!({
        "access_token": mcp_access_token,
        "token_type": "Bearer",
        "expires_in": access_token_ttl(),
        "refresh_token": mcp_refresh_token,
    }))
    .into_response()
}

async fn handle_refresh_token_grant(state: AppState, form: &mut TokenForm) -> Response {
    let refresh_token = match &form.refresh_token {
        Some(rt) => rt.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "invalid_request", "error_description": "refresh_token required"})),
            )
                .into_response()
        }
    };

    // Look up the refresh token
    let creds = match state.db.get_refresh_token(&refresh_token).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "invalid_grant"})),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("DB error: {e}")})),
            )
                .into_response()
        }
    };

    let (_halo_at, halo_rt) = creds;

    // Refresh the underlying HaloPSA token. We must not silently reuse the
    // old HaloPSA access token on failure — that would hand back a valid
    // MCP-level token wrapping a dead HaloPSA token, so the client thinks
    // re-authentication succeeded while every subsequent HaloPSA API call
    // keeps failing with 401. Treat any failure as invalid_grant so the
    // client is forced through a full re-authorization instead.
    if halo_rt.is_empty() {
        let _ = state.db.delete_refresh_token(&refresh_token).await;
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "invalid_grant",
                "error_description": "No HaloPSA refresh token available; re-authorization required"
            })),
        )
            .into_response();
    }

    let (new_halo_at, new_halo_rt) = match refresh_halo_token(&state, &halo_rt).await {
        Ok((at, rt)) => (at, rt),
        Err(e) => {
            eprintln!("HaloPSA token refresh failed: {e}");
            let _ = state.db.delete_refresh_token(&refresh_token).await;
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "invalid_grant",
                    "error_description": "HaloPSA token refresh failed; re-authorization required"
                })),
            )
                .into_response();
        }
    };

    // Rotate: delete old, issue new
    let _ = state.db.delete_refresh_token(&refresh_token).await;

    let new_mcp_access = uuid::Uuid::new_v4().to_string();
    let new_mcp_refresh = uuid::Uuid::new_v4().to_string();

    if let Err(e) = state
        .db
        .insert_access_token(&new_mcp_access, &new_halo_at, &new_halo_rt)
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Token storage failed: {e}")})),
        )
            .into_response();
    }

    if let Err(e) = state
        .db
        .insert_refresh_token(&new_mcp_refresh, &new_halo_at, &new_halo_rt)
        .await
    {
        eprintln!("Failed to store refresh token: {e}");
    }

    Json(json!({
        "access_token": new_mcp_access,
        "token_type": "Bearer",
        "expires_in": access_token_ttl(),
        "refresh_token": new_mcp_refresh,
    }))
    .into_response()
}

/// Refresh a HaloPSA token using the refresh_token grant.
async fn refresh_halo_token(
    state: &AppState,
    halo_refresh_token: &str,
) -> Result<(String, String), String> {
    let params = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", halo_refresh_token),
        ("client_id", &state.halo_client_id),
        ("client_secret", &state.halo_client_secret),
    ];

    let resp = state
        .http_client
        .post(format!("{}/auth/token", state.halo_url))
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Refresh failed: {e}"))?;

    if !resp.status().is_success() {
        return Err("HaloPSA refresh failed".into());
    }

    let data: Value = resp.json().await.map_err(|e| format!("Parse failed: {e}"))?;

    let at = data
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or("No access_token")?
        .to_string();
    let rt = data
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Ok((at, rt))
}

/// Dynamic client registration: POST /register
pub async fn handle_register(
    State(state): State<AppState>,
    _headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    if state.register_rate_limit.lock().await.check().is_err() {
        return (StatusCode::TOO_MANY_REQUESTS, Json(json!({"error": "rate_limited"}))).into_response();
    }

    let redirect_uris = body
        .get("redirect_uris")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if redirect_uris.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_client_metadata", "error_description": "redirect_uris is required"})),
        )
            .into_response();
    }
    // Reject non-https / non-loopback redirect URIs so a registered client
    // can't point /authorize's redirect at plaintext http (or something
    // that isn't a URL at all).
    for uri in &redirect_uris {
        let is_https = uri.starts_with("https://");
        let is_loopback = uri.starts_with("http://localhost")
            || uri.starts_with("http://127.0.0.1")
            || uri.starts_with("http://[::1]");
        if !is_https && !is_loopback {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "invalid_redirect_uri", "error_description": "redirect_uris must be https:// (or http://localhost for local development)"})),
            )
                .into_response();
        }
    }

    let client_name = body
        .get("client_name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let client_id = uuid::Uuid::new_v4().to_string();

    state.registered_clients.write().await.insert(
        client_id.clone(),
        RegisteredClient {
            redirect_uris: redirect_uris.clone(),
            created_at: Instant::now(),
        },
    );

    eprintln!("Registered client: {client_name} ({client_id})");

    (
        StatusCode::CREATED,
        Json(json!({
            "client_id": client_id,
            "client_name": client_name,
            "redirect_uris": redirect_uris,
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
            "token_endpoint_auth_method": "none",
        })),
    )
        .into_response()
}

// URL encoding helper
mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut result = String::with_capacity(s.len() * 3);
        for byte in s.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    result.push(byte as char);
                }
                _ => {
                    result.push_str(&format!("%{:02X}", byte));
                }
            }
        }
        result
    }
}
