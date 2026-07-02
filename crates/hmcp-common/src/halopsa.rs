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

/// Build a HaloPSA advanced_search value: a JSON array of filter objects.
/// filter_type 4 is "contains", confirmed against the agent UI's own
/// request for a client name search.
fn advanced_search_filter(filter_name: &str, value: &str) -> String {
    json!([{
        "filter_name": filter_name,
        "filter_type": 4,
        "filter_value": value,
    }])
    .to_string()
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

    /// Create an action (note/reply) on a ticket. `status_id`, when set,
    /// requests a ticket status change alongside the note — mirrors how
    /// HaloPSA's own agent UI submits a note and a status change together
    /// as a single action.
    pub async fn create_action(
        &self,
        ticket_id: i64,
        note: &str,
        outcome: &str,
        workflow_action_id: Option<i64>,
        status_id: Option<i64>,
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
        if let Some(sid) = status_id {
            action["status_id"] = json!(sid);
        }
        self.post("/api/Actions", &json!([action])).await
    }

    /// Log time against a ticket by creating a time-entry action.
    /// `time_minutes` is converted to decimal hours (timetaken field).
    /// HaloPSA requires an outcome on every action ("An Outcome must be
    /// entered for this Action"), same requirement create_action already
    /// satisfies with its "note" default.
    pub async fn log_time(
        &self,
        ticket_id: i64,
        time_minutes: f64,
        note: &str,
        outcome: &str,
        hidden_from_user: bool,
    ) -> Result<Value, String> {
        let timetaken = time_minutes / 60.0;
        let body = json!({
            "ticket_id": ticket_id,
            "note": note,
            "outcome": outcome,
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

    /// Delete an action by ID. HaloPSA requires ticket_id ("ticket_id must
    /// be included when deleting an Action"), same as get_action/update_action.
    pub async fn delete_action(&self, ticket_id: i64, action_id: i64) -> Result<(), String> {
        self.delete(
            &format!("/api/Actions/{action_id}"),
            &[("ticket_id", ticket_id.to_string())],
        )
        .await
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

    /// List statuses, optionally filtered by type (e.g. "ticket").
    /// Confirmed against the agent UI's own request — omitting the filter
    /// returns all statuses regardless of type.
    pub async fn list_statuses(&self, status_type: Option<&str>) -> Result<Vec<Value>, String> {
        let mut params: Vec<(&str, String)> = Vec::new();
        if let Some(t) = status_type {
            params.push(("type", t.to_string()));
        }
        let value = self.get_raw("/api/Status", &params).await?;
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

    /// List clients with optional keyword search (by name).
    pub async fn list_clients(
        &self,
        page: i64,
        page_size: i64,
        search: Option<&str>,
    ) -> Result<(Vec<Value>, i64), String> {
        let mut params: Vec<(&str, String)> = vec![
            // HaloPSA ignores page_size on this endpoint unless pageinate=true
            // is also set, confirmed against the agent UI's own request.
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(100).to_string()),
            ("includecolumns", "true".into()),
        ];
        if let Some(s) = search {
            // A plain "search" param is silently ignored on /api/Client —
            // name filtering requires advanced_search, a JSON-encoded array
            // of filter objects, confirmed against the agent UI's own request.
            params.push(("advanced_search", advanced_search_filter("name", s)));
        }
        let value = self.get_raw("/api/Client", &params).await?;
        let record_count = value.get("record_count").and_then(|v| v.as_i64()).unwrap_or(0);
        let records = parse_halo_list::<Value>(value);
        Ok((records, record_count))
    }

    /// Search clients by keyword.
    pub async fn search_clients(&self, query: &str, page_size: i64) -> Result<(Vec<Value>, i64), String> {
        self.list_clients(1, page_size, Some(query)).await
    }

    /// List users (end-user contacts), optionally filtered by client and/or keyword.
    pub async fn list_users(
        &self,
        page: i64,
        page_size: i64,
        client_id: Option<i64>,
        search: Option<&str>,
    ) -> Result<(Vec<Value>, i64), String> {
        let mut params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(100).to_string()),
            // Confirmed against the agent UI's Users tab request — without
            // these, results are liable to include agents/service accounts
            // rather than genuine end-user contacts.
            ("onlyusers", "true".into()),
            ("includeserviceaccount", "true".into()),
            ("includenonserviceaccount", "true".into()),
            ("exclude_generaluser", "false".into()),
            ("exclude_agents", "false".into()),
            ("exclude_defaultsiteusers", "false".into()),
        ];
        if let Some(id) = client_id {
            params.push(("client_id", id.to_string()));
        }
        if let Some(s) = search {
            // Confirmed against the agent UI's own search request: Users
            // uses a plain search param, unlike Clients' advanced_search —
            // each HaloPSA list endpoint has its own convention, don't
            // assume one carries over to another.
            params.push(("search", s.to_string()));
        }
        let value = self.get_raw("/api/Users", &params).await?;
        let record_count = value.get("record_count").and_then(|v| v.as_i64()).unwrap_or(0);
        let records = parse_halo_list::<Value>(value);
        Ok((records, record_count))
    }

    /// Search users by keyword.
    pub async fn search_users(&self, query: &str, page_size: i64) -> Result<(Vec<Value>, i64), String> {
        self.list_users(1, page_size, None, Some(query)).await
    }

    /// Get a single user (end-user contact) by ID.
    pub async fn get_user(&self, user_id: i64) -> Result<Value, String> {
        self.get_raw(
            &format!("/api/Users/{user_id}"),
            &[("includedetails", "true".into())],
        )
        .await
    }

    /// List saved report definitions within a category (reportgroup_id=0
    /// is "All Reports"), or search by name across all categories.
    /// Confirmed against the agent UI: search uses a plain `search` param
    /// (unlike Clients/Users, which need advanced_search) and sends
    /// reportgroup_id as the literal string "null" when searching —
    /// distinct from reportgroup_id=0, and spans every category.
    pub async fn list_reports(
        &self,
        page: i64,
        page_size: i64,
        reportgroup_id: i64,
        search: Option<&str>,
    ) -> Result<(Vec<Value>, i64), String> {
        let mut params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(100).to_string()),
            ("order", "name".into()),
            ("orderdesc", "false".into()),
            ("type", "0".into()),
        ];
        if let Some(s) = search {
            params.push(("reportgroup_id", "null".into()));
            params.push(("search", s.to_string()));
        } else {
            params.push(("reportgroup_id", reportgroup_id.to_string()));
        }
        let value = self.get_raw("/api/Report", &params).await?;
        let record_count = value.get("record_count").and_then(|v| v.as_i64()).unwrap_or(0);
        let records = parse_halo_list::<Value>(value);
        Ok((records, record_count))
    }

    /// List report categories (groups) — confirmed against the agent UI as
    /// a generic lookup table (lookupid=41), not a Report-specific endpoint.
    pub async fn list_report_categories(&self) -> Result<Vec<Value>, String> {
        self.get_lookup_values(41, true).await
    }

    /// Get values for any HaloPSA lookup table by ID. Generalizes the
    /// lookupid=41 (report categories) pattern confirmed against the agent
    /// UI — the same /api/Lookup endpoint serves other lookup tables too
    /// (e.g. priorities, outcomes), keyed by a different lookupid per table.
    pub async fn get_lookup_values(&self, lookupid: i64, istree: bool) -> Result<Vec<Value>, String> {
        let value = self
            .get_raw(
                "/api/Lookup",
                &[("lookupid", lookupid.to_string()), ("istree", istree.to_string())],
            )
            .await?;
        Ok(parse_halo_list::<Value>(value))
    }

    /// List the steps (and their available transition actions) for a
    /// workflow. Thin wrapper over the existing get_workflow — no new
    /// endpoint, just exposing data we already fetch.
    pub async fn list_workflow_steps(&self, workflow_id: i64) -> Result<Vec<WorkflowStep>, String> {
        let workflow = self.get_workflow(workflow_id).await?;
        Ok(workflow.steps)
    }

    /// Run a saved report by ID and return the full response, including
    /// the executed rows under `report.rows`. `filters` overrides the
    /// report's saved filters — confirmed against the agent UI's own
    /// request when changing a report filter and re-running: a JSON array
    /// of `{fieldname, stringruletype, stringrulevalues}` objects (the
    /// same shape as the `filters` field in a report's own definition).
    /// `extra_params` carries any other report-specific query params
    /// (e.g. a report might take "clientname") passed through generically.
    pub async fn run_report(
        &self,
        report_id: i64,
        filters: &[Value],
        extra_params: &[(String, String)],
    ) -> Result<Value, String> {
        let mut params: Vec<(&str, String)> = vec![
            ("includedetails", "true".into()),
            ("loadreport", "true".into()),
            ("dontloadsystemreport", "false".into()),
        ];
        if !filters.is_empty() {
            params.push(("filters", json!(filters).to_string()));
        }
        for (k, v) in extra_params {
            params.push((k.as_str(), v.clone()));
        }
        self.get_raw(&format!("/api/Report/{report_id}"), &params).await
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

    /// Get a single asset by ID. Confirmed against the agent UI.
    pub async fn get_asset(&self, asset_id: i64) -> Result<Value, String> {
        self.get_raw(
            &format!("/api/Asset/{asset_id}"),
            &[("includedetails", "true".into())],
        )
        .await
    }

    /// List assets, optionally filtered by type and/or keyword search.
    /// Confirmed against the agent UI: assettype_id is simply omitted for
    /// "All Assets" (no sentinel value needed, unlike Reports' reportgroup_id).
    /// Search sends both globalSearchID and search set to the same term.
    pub async fn list_assets(
        &self,
        page: i64,
        page_size: i64,
        assettype_id: Option<i64>,
        search: Option<&str>,
    ) -> Result<(Vec<Value>, i64), String> {
        let mut params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(100).to_string()),
            ("includeinactive", "false".into()),
            ("includechildren", "true".into()),
            ("convert_date_assetfields_to_iso", "true".into()),
            ("convert_int_assetfield", "true".into()),
        ];
        if let Some(id) = assettype_id {
            params.push(("assettype_id", id.to_string()));
        }
        if let Some(s) = search {
            params.push(("globalSearchID", s.to_string()));
            params.push(("search", s.to_string()));
        }
        let value = self.get_raw("/api/asset", &params).await?;
        let record_count = value.get("record_count").and_then(|v| v.as_i64()).unwrap_or(0);
        let records = parse_halo_list::<Value>(value);
        Ok((records, record_count))
    }

    /// Search assets by keyword across all types.
    pub async fn search_assets(&self, query: &str, page_size: i64) -> Result<(Vec<Value>, i64), String> {
        self.list_assets(1, page_size, None, Some(query)).await
    }

    /// List sites. Confirmed against the agent UI — plain pageinate
    /// convention, no unusual params.
    pub async fn list_sites(&self, page: i64, page_size: i64) -> Result<(Vec<Value>, i64), String> {
        let params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(100).to_string()),
        ];
        let value = self.get_raw("/api/site", &params).await?;
        let record_count = value.get("record_count").and_then(|v| v.as_i64()).unwrap_or(0);
        let records = parse_halo_list::<Value>(value);
        Ok((records, record_count))
    }

    /// Get a single site by ID. Confirmed against the agent UI.
    pub async fn get_site(&self, site_id: i64) -> Result<Value, String> {
        self.get_raw(
            &format!("/api/Site/{site_id}"),
            &[("includedetails", "true".into())],
        )
        .await
    }

    /// Create a new site. `issitedetails: true` is required — confirmed
    /// against the agent UI, Site writes need it to indicate this payload
    /// is the site-details form (Halo entities can have multiple detail
    /// sub-forms on the same base object).
    pub async fn create_site(&self, mut site: Value) -> Result<Value, String> {
        if let Some(obj) = site.as_object_mut() {
            obj.insert("issitedetails".into(), json!(true));
        }
        self.post("/api/Site", &json!([site])).await
    }

    /// Update an existing site. Same issitedetails requirement as create.
    pub async fn update_site(&self, site_id: i64, mut fields: Value) -> Result<Value, String> {
        if let Some(obj) = fields.as_object_mut() {
            obj.insert("id".into(), json!(site_id));
            obj.insert("issitedetails".into(), json!(true));
        }
        self.post("/api/Site", &json!([fields])).await
    }

    /// List agents (staff members). Same base path as the existing,
    /// already-confirmed get_agent single-record endpoint.
    pub async fn list_agents(&self, page: i64, page_size: i64) -> Result<(Vec<Value>, i64), String> {
        let params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(100).to_string()),
        ];
        let value = self.get_raw("/api/Agent", &params).await?;
        let record_count = value.get("record_count").and_then(|v| v.as_i64()).unwrap_or(0);
        let records = parse_halo_list::<Value>(value);
        Ok((records, record_count))
    }

    /// List asset groups. Endpoint path follows HaloPSA's singular
    /// resource-name convention (TicketType, AssetType) — NOT confirmed
    /// against a real capture, flag for retest.
    pub async fn list_asset_groups(&self, page: i64, page_size: i64) -> Result<(Vec<Value>, i64), String> {
        let params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(100).to_string()),
        ];
        let value = self.get_raw("/api/AssetGroup", &params).await?;
        let record_count = value.get("record_count").and_then(|v| v.as_i64()).unwrap_or(0);
        let records = parse_halo_list::<Value>(value);
        Ok((records, record_count))
    }

    /// Get a single asset group by ID. Same endpoint-guess caveat as
    /// list_asset_groups.
    pub async fn get_asset_group(&self, group_id: i64) -> Result<Value, String> {
        self.get_raw(
            &format!("/api/AssetGroup/{group_id}"),
            &[("includedetails", "true".into())],
        )
        .await
    }

    /// List SLAs. Confirmed against the agent UI's own request.
    pub async fn list_slas(&self) -> Result<Vec<Value>, String> {
        let value = self
            .get_raw(
                "/api/SLA",
                &[
                    ("showall", "true".into()),
                    ("access_control_level", "2".into()),
                    ("isconfig", "true".into()),
                ],
            )
            .await?;
        Ok(parse_halo_list::<Value>(value))
    }

    /// Get a single SLA by ID, including its nested priority levels.
    /// Confirmed against the agent UI's own request.
    pub async fn get_sla(&self, sla_id: i64) -> Result<Value, String> {
        self.get_raw(
            &format!("/api/SLA/{sla_id}"),
            &[("includedetails", "true".into())],
        )
        .await
    }

    /// List action outcomes. Confirmed against the agent UI's own request.
    pub async fn list_outcomes(&self) -> Result<Vec<Value>, String> {
        let value = self
            .get_raw(
                "/api/Outcome",
                &[("showhidden", "true".into()), ("access_control_level", "2".into())],
            )
            .await?;
        Ok(parse_halo_list::<Value>(value))
    }

    /// List categories of a given type. Confirmed against the agent UI:
    /// type_id=1 is ticket categories, type_id=2 is resolution categories —
    /// there are reportedly 4 category types total on this instance, so
    /// type_id is a required param rather than hardcoding just these two.
    pub async fn list_categories(&self, type_id: i64) -> Result<Vec<Value>, String> {
        let value = self
            .get_raw(
                "/api/Category",
                &[("showall", "true".into()), ("type_id", type_id.to_string())],
            )
            .await?;
        Ok(parse_halo_list::<Value>(value))
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

    async fn delete(&self, path: &str, params: &[(&str, String)]) -> Result<(), String> {
        self.rate_limit.lock().await.check()?;

        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .delete(&url)
            .bearer_auth(&self.access_token)
            .query(params)
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
