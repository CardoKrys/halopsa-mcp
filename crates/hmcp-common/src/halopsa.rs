use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine;
use reqwest::Client;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use zeroize::Zeroize;

use crate::types::*;

/// Decode a JWT's payload claims without verifying the signature. Only
/// safe for display purposes — never use this for authorization decisions.
/// Returns None if the token isn't a 3-part JWT or the payload isn't JSON.
fn decode_jwt_claims(token: &str) -> Option<Value> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[1])
        .ok()?;
    serde_json::from_slice(&payload).ok()
}

const MAX_RESPONSE_SIZE: u64 = 50 * 1024 * 1024; // 50MB
const RATE_LIMIT_REQUESTS: u32 = 400;
const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(300); // 5 minutes

/// HaloPSA API client authenticated with a Bearer token.
/// The token comes from either OAuth Authorization Code (user) or Client Credentials (service).
pub struct HaloPSAClient {
    base_url: String,
    access_token: String,
    http: Client,
    rate_limit: Arc<Mutex<RateTracker>>,
}

struct RateTracker {
    count: u32,
    window_start: Instant,
}

impl RateTracker {
    fn new() -> Self {
        Self {
            count: 0,
            window_start: Instant::now(),
        }
    }

    fn check(&mut self) -> Result<(), String> {
        if self.window_start.elapsed() > RATE_LIMIT_WINDOW {
            self.count = 0;
            self.window_start = Instant::now();
        }
        if self.count >= RATE_LIMIT_REQUESTS {
            return Err("Rate limit exceeded (400 requests per 5 minutes)".into());
        }
        self.count += 1;
        Ok(())
    }
}

impl Drop for HaloPSAClient {
    fn drop(&mut self) {
        self.access_token.zeroize();
    }
}

impl HaloPSAClient {
    pub fn new(base_url: &str, access_token: &str, http: Client) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            access_token: access_token.to_string(),
            http,
            rate_limit: Arc::new(Mutex::new(RateTracker::new())),
        }
    }

    /// Validate the token by making a cheap API call.
    pub async fn validate(&self) -> Result<(), String> {
        self.get_raw("/api/Status", &[("count", "1".into())]).await?;
        Ok(())
    }

    // --- Tickets ---

    /// List tickets with optional filters.
    pub async fn list_tickets(
        &self,
        page: i64,
        page_size: i64,
        filters: &TicketFilter,
    ) -> Result<(Vec<Value>, i64), String> {
        let mut params: Vec<(&str, String)> = vec![
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(100).to_string()),
            ("includecolumns", "true".into()),
        ];

        if let Some(ref search) = filters.search {
            params.push(("search", search.clone()));
        }
        if let Some(id) = filters.client_id {
            params.push(("client_id", id.to_string()));
        }
        if let Some(id) = filters.agent_id {
            params.push(("agent_id", id.to_string()));
        }
        if let Some(id) = filters.team_id {
            params.push(("team_id", id.to_string()));
        }
        if let Some(id) = filters.status_id {
            params.push(("status_id", id.to_string()));
        }
        if let Some(id) = filters.tickettype_id {
            params.push(("tickettype_id", id.to_string()));
        }
        if filters.open_only {
            params.push(("open_only", "true".into()));
        }

        let value = self
            .get_raw("/api/Tickets", &params)
            .await?;

        let record_count = value.get("record_count")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let records = parse_halo_list::<Value>(value);

        Ok((records, record_count))
    }

    /// Get a single ticket by ID with full details.
    pub async fn get_ticket(&self, ticket_id: i64) -> Result<Value, String> {
        self.get_raw(
            &format!("/api/Tickets/{ticket_id}"),
            &[("includedetails", "true".into())],
        )
        .await
    }

    /// Check if the user can access a ticket. Returns true if accessible.
    pub async fn can_access_ticket(&self, ticket_id: i64) -> bool {
        self.get_no_params(&format!("/api/Tickets/{ticket_id}"))
            .await
            .is_ok()
    }

    /// Create a new ticket.
    pub async fn create_ticket(&self, ticket: Value) -> Result<Value, String> {
        self.post("/api/Tickets", &json!([ticket])).await
    }

    /// Update an existing ticket.
    pub async fn update_ticket(&self, ticket_id: i64, fields: Value) -> Result<Value, String> {
        let mut body = fields;
        if let Some(obj) = body.as_object_mut() {
            obj.insert("id".into(), json!(ticket_id));
        }
        self.post("/api/Tickets", &json!([body])).await
    }

    // --- Actions ---

    /// List actions on a ticket.
    pub async fn list_actions(&self, ticket_id: i64, include_private: bool) -> Result<Vec<Value>, String> {
        let params: Vec<(&str, String)> = vec![
            ("ticket_id", ticket_id.to_string()),
            ("excludeprivate", (!include_private).to_string()),
            ("includehtmlnote", "true".into()),
            ("includeagent", "true".into()),
            ("includeattachments", "true".into()),
        ];
        let value = self.get_raw("/api/Actions", &params).await?;
        Ok(parse_halo_list::<Value>(value))
    }

    /// Create an action (note/reply) on a ticket, optionally executing a
    /// workflow transition via `workflow_action_id`. Ticket status is
    /// workflow-driven on this instance (confirmed on multiple ticket
    /// types) — putting `status_id` directly on the ticket or the action
    /// payload is silently ignored. The only confirmed way to change
    /// status is a workflow transition, i.e. `workflow_action_id`.
    pub async fn create_action(
        &self,
        ticket_id: i64,
        note: &str,
        outcome: &str,
        workflow_action_id: Option<i64>,
        hidden_from_user: bool,
    ) -> Result<Value, String> {
        let mut action = json!({
            "ticket_id": ticket_id,
            "note": note,
            "outcome": outcome,
            "hiddenfromuser": hidden_from_user,
        });
        if let Some(wf_id) = workflow_action_id {
            action["workflow_subdetail_id"] = json!(wf_id);
        }
        self.post("/api/Actions", &json!([action])).await
    }

    /// Log time against a ticket by creating a time-entry action.
    /// `time_minutes` is converted to decimal hours (timetaken field).
    pub async fn log_time(
        &self,
        ticket_id: i64,
        time_minutes: f64,
        note: &str,
        hidden_from_user: bool,
    ) -> Result<Value, String> {
        let timetaken = time_minutes / 60.0;
        let body = json!({
            "ticket_id": ticket_id,
            "note": note,
            "timetaken": timetaken,
            "hiddenfromuser": hidden_from_user,
        });
        self.post("/api/Actions", &json!([body])).await
    }

    /// Update an existing action's note text. HaloPSA requires ticket_id on
    /// the action update payload ("An Action must be associated with a
    /// ticket_id"), even though the action_id alone identifies the record.
    pub async fn update_action(
        &self,
        ticket_id: i64,
        action_id: i64,
        note: &str,
        hidden_from_user: Option<bool>,
    ) -> Result<Value, String> {
        let mut body = json!({ "id": action_id, "ticket_id": ticket_id, "note": note });
        if let Some(hidden) = hidden_from_user {
            body["hiddenfromuser"] = json!(hidden);
        }
        self.post("/api/Actions", &json!([body])).await
    }

    /// Delete an action by ID.
    pub async fn delete_action(&self, action_id: i64) -> Result<(), String> {
        self.delete(&format!("/api/Actions/{action_id}")).await
    }

    /// Get a single action by ID. HaloPSA requires ticket_id as a query
    /// param ("ticket_id must be specified") even when fetching by action_id.
    pub async fn get_action(&self, ticket_id: i64, action_id: i64) -> Result<Value, String> {
        self.get_raw(
            &format!("/api/Actions/{action_id}"),
            &[
                ("ticket_id", ticket_id.to_string()),
                ("includedetails", "true".into()),
            ],
        )
        .await
    }

    // --- Workflows ---

    /// Get a workflow by ID with full details (steps, stages, actions).
    pub async fn get_workflow(&self, workflow_id: i64) -> Result<Workflow, String> {
        let value = self
            .get_raw(
                &format!("/api/Workflow/{workflow_id}"),
                &[("includedetails", "true".into())],
            )
            .await?;
        serde_json::from_value(value).map_err(|e| format!("Failed to parse workflow: {e}"))
    }

    /// Get ticket type by ID (includes workflow definition if present).
    pub async fn get_ticket_type(&self, type_id: i64) -> Result<Value, String> {
        self.get_no_params(&format!("/api/TicketType/{type_id}")).await
    }

    /// Get available workflow actions for a ticket's current step.
    /// Filters to actions available to agents (agentaction >= 1).
    pub async fn get_available_actions(&self, ticket_id: i64) -> Result<Vec<WorkflowAction>, String> {
        let ticket = self.get_ticket(ticket_id).await?;
        let type_id = ticket.get("tickettype_id")
            .and_then(|v| v.as_i64())
            .ok_or("Ticket has no tickettype_id")?;
        let current_step = ticket.get("workflow_step").and_then(|v| v.as_i64());

        let ticket_type = self.get_ticket_type(type_id).await?;
        let workflow_id = ticket_type.get("workflow_id").and_then(|v| v.as_i64());

        let Some(workflow_id) = workflow_id else {
            return Ok(Vec::new());
        };

        let workflow = self.get_workflow(workflow_id).await?;
        let mut available = Vec::new();

        for step in &workflow.steps {
            for action in &step.actions {
                // Filter: agent actions only (agentaction >= 1 or no restriction)
                let agent_ok = action.agentaction.map_or(true, |a| a >= 1);
                if !agent_ok {
                    continue;
                }

                // Filter: must match current step
                if let Some(cs) = current_step {
                    if action.start_step != Some(cs) && action.start_step.is_some() {
                        continue;
                    }
                }

                available.push(action.clone());
            }
        }

        Ok(available)
    }

    // --- Supporting lookups ---

    /// List all statuses.
    pub async fn list_statuses(&self) -> Result<Vec<Value>, String> {
        let value = self.get_no_params("/api/Status").await?;
        Ok(parse_halo_list::<Value>(value))
    }

    /// List teams/queues.
    pub async fn list_teams(&self) -> Result<Vec<Value>, String> {
        let value = self.get_no_params("/api/Team").await?;
        Ok(parse_halo_list::<Value>(value))
    }

    /// Get a client by ID.
    pub async fn get_client(&self, client_id: i64) -> Result<Value, String> {
        self.get_no_params(&format!("/api/Client/{client_id}")).await
    }

    /// List ticket types.
    pub async fn list_ticket_types(&self) -> Result<Vec<Value>, String> {
        let value = self.get_no_params("/api/TicketType").await?;
        Ok(parse_halo_list::<Value>(value))
    }

    /// Get agent (current user) info.
    pub async fn get_agent(&self, agent_id: i64) -> Result<Value, String> {
        self.get_no_params(&format!("/api/Agent/{agent_id}")).await
    }

    /// Get current user info. /api/AuthInfo only returns server/tenant
    /// metadata (auth_url, integrationServiceUrl, tenant_id) on this
    /// instance, not the agent's own identity. HaloPSA's OAuth server
    /// issues JWT access tokens, so as a best-effort enrichment we decode
    /// the token's claims (no signature verification needed — we only use
    /// this for display, HaloPSA itself still enforces authorization on
    /// every request) to surface agent id/name/email when present.
    pub async fn get_me(&self) -> Result<Value, String> {
        let mut result = self.get_no_params("/api/AuthInfo").await?;
        if let Some(claims) = decode_jwt_claims(&self.access_token) {
            if let Some(obj) = result.as_object_mut() {
                for key in ["sub", "agentid", "agent_id", "name", "email", "preferred_username"] {
                    if let Some(v) = claims.get(key) {
                        obj.insert(key.to_string(), v.clone());
                    }
                }
            }
        }
        Ok(result)
    }

    /// Search tickets by keyword.
    pub async fn search_tickets(&self, query: &str, page_size: i64) -> Result<(Vec<Value>, i64), String> {
        self.list_tickets(1, page_size, &TicketFilter {
            search: Some(query.to_string()),
            ..Default::default()
        }).await
    }

    // --- Assets ---

    /// List assets associated with a ticket.
    pub async fn get_ticket_assets(&self, ticket_id: i64) -> Result<Vec<Value>, String> {
        let value = self.get_raw("/api/Asset", &[("ticket_id", ticket_id.to_string())]).await?;
        Ok(parse_halo_list(value))
    }

    // --- Internal HTTP methods ---

    async fn get_raw(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<Value, String> {
        self.rate_limit.lock().await.check()?;

        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&self.access_token)
            .query(params)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {e}"))?;

        let status = resp.status();
        if let Some(len) = resp.content_length() {
            if len > MAX_RESPONSE_SIZE {
                return Err(format!("Response too large: {len} bytes"));
            }
        }

        let body = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {e}"))?;

        if !status.is_success() {
            let preview = if body.len() > 500 { &body[..500] } else { &body };
            return Err(format!("HaloPSA API error {status}: {preview}"));
        }

        serde_json::from_str(&body).map_err(|e| format!("Failed to parse JSON: {e}"))
    }

    /// GET with no query params.
    async fn get_no_params(&self, path: &str) -> Result<Value, String> {
        self.get_raw(path, &[]).await
    }

    async fn delete(&self, path: &str) -> Result<(), String> {
        self.rate_limit.lock().await.check()?;

        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .delete(&url)
            .bearer_auth(&self.access_token)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {e}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            let preview = if body.len() > 500 { &body[..500] } else { &body };
            return Err(format!("HaloPSA API error {status}: {preview}"));
        }

        Ok(())
    }

    async fn post(&self, path: &str, body: &Value) -> Result<Value, String> {
        self.rate_limit.lock().await.check()?;

        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.access_token)
            .json(body)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {e}"))?;

        let status = resp.status();
        let body_text = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {e}"))?;

        if !status.is_success() {
            let preview = if body_text.len() > 500 {
                &body_text[..500]
            } else {
                &body_text
            };
            return Err(format!("HaloPSA API error {status}: {preview}"));
        }

        serde_json::from_str(&body_text).map_err(|e| format!("Failed to parse JSON: {e}"))
    }

}

/// Filters for listing tickets.
#[derive(Debug, Default)]
pub struct TicketFilter {
    pub search: Option<String>,
    pub client_id: Option<i64>,
    pub agent_id: Option<i64>,
    pub team_id: Option<i64>,
    pub status_id: Option<i64>,
    pub tickettype_id: Option<i64>,
    pub open_only: bool,
}

// --- OAuth token management for service account ---

/// Manages a HaloPSA Client Credentials token (for the embedding service account).
pub struct ServiceTokenManager {
    halo_url: String,
    client_id: String,
    client_secret: String,
    tenant: Option<String>,
    http: Client,
    token: Mutex<Option<CachedToken>>,
}

struct CachedToken {
    access_token: String,
    expires_at: Instant,
}

impl Drop for CachedToken {
    fn drop(&mut self) {
        self.access_token.zeroize();
    }
}

impl ServiceTokenManager {
    pub fn new(
        halo_url: &str,
        client_id: &str,
        client_secret: &str,
        tenant: Option<&str>,
        http: Client,
    ) -> Self {
        Self {
            halo_url: halo_url.trim_end_matches('/').to_string(),
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            tenant: tenant.map(|s| s.to_string()),
            http,
            token: Mutex::new(None),
        }
    }

    /// Get a valid access token, refreshing if expired.
    pub async fn get_token(&self) -> Result<String, String> {
        let mut guard = self.token.lock().await;

        // Return cached token if still valid (with 5-minute buffer)
        if let Some(ref cached) = *guard {
            if cached.expires_at > Instant::now() + Duration::from_secs(300) {
                return Ok(cached.access_token.clone());
            }
        }

        // Request new token
        let mut params = vec![
            ("grant_type", "client_credentials"),
            ("client_id", &self.client_id),
            ("client_secret", &self.client_secret),
            ("scope", "all"),
        ];
        let tenant_val;
        if let Some(ref t) = self.tenant {
            tenant_val = t.clone();
            params.push(("tenant", &tenant_val));
        }

        let resp = self
            .http
            .post(format!("{}/auth/token", self.halo_url))
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Token request failed: {e}"))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Token request failed: {body}"));
        }

        let data: Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse token response: {e}"))?;

        let access_token = data
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or("No access_token in response")?
            .to_string();

        let expires_in = data
            .get("expires_in")
            .and_then(|v| v.as_u64())
            .unwrap_or(3600);

        *guard = Some(CachedToken {
            access_token: access_token.clone(),
            expires_at: Instant::now() + Duration::from_secs(expires_in),
        });

        Ok(access_token)
    }

    /// Create a HaloPSAClient using the service account token.
    pub async fn client(&self) -> Result<HaloPSAClient, String> {
        let token = self.get_token().await?;
        Ok(HaloPSAClient::new(&self.halo_url, &token, self.http.clone()))
    }
}
