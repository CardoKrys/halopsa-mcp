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
            // HaloPSA ignores page_size on this endpoint unless pageinate=true
            // is also set (same quirk confirmed on /api/Client).
            ("pageinate", "true".into()),
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
        if let Some(ref priority) = filters.priority {
            // Confirmed against the agent UI: /api/Tickets supports the
            // same advanced_search mechanism as Clients — priority is
            // filtered by label (e.g. "P3", "High"), not a numeric ID.
            // Priority labels are environment-specific (Sandbox uses
            // P1-P5, Production uses named levels like High/Critical/RFO).
            params.push(("advanced_search", advanced_search_filter("priority", priority)));
        }
        if let Some(id) = filters.ticketarea_id {
            // Confirmed against the agent UI's own request (Projects nav
            // is literally /api/Tickets?ticketarea_id=<Projects area id>).
            params.push(("ticketarea_id", id.to_string()));
        }
        if let Some(id) = filters.parent_id {
            // Parent/child ticket relationship field name unconfirmed
            // against a real capture — flag for retest.
            params.push(("parent_id", id.to_string()));
        }

        let value = self
            .get_raw("/api/Tickets", &params)
            .await?;

        let record_count = value.get("record_count")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let mut records = parse_halo_list::<Value>(value);
        // Some HaloPSA instances/endpoints ignore page_size server-side even
        // with pageinate=true set — truncate client-side as a safety net.
        records.truncate(page_size.max(1) as usize);

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
    /// Confirmed against the agent UI's own request — includedetails,
    /// isrtconfig and includeconfig together return the full config
    /// (including the field list shown on the "Field List" tab, which
    /// fires no separate API call of its own — it's rendered client-side
    /// from this same response).
    pub async fn get_ticket_type(&self, type_id: i64) -> Result<Value, String> {
        self.get_raw(
            &format!("/api/TicketType/{type_id}"),
            &[
                ("includedetails", "true".into()),
                ("isrtconfig", "true".into()),
                ("includeconfig", "true".into()),
            ],
        )
        .await
    }

    /// List the fields configured for a ticket type (the "Field List" tab
    /// in the agent UI). The exact JSON field name wrapping this array in
    /// the ticket type response is NOT confirmed — same caveat as
    /// list_priorities. Tries the likely "fields" key first, falling back
    /// to the first array field found.
    pub async fn list_ticket_type_fields(&self, type_id: i64) -> Result<Vec<Value>, String> {
        let tt = self.get_ticket_type(type_id).await?;
        if let Some(arr) = tt.get("fields").and_then(|v| v.as_array()) {
            return Ok(arr.clone());
        }
        if let Some(obj) = tt.as_object() {
            if let Some(arr) = obj.values().find_map(|v| v.as_array()) {
                return Ok(arr.clone());
            }
        }
        Ok(Vec::new())
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

    /// Get full configuration details for a single status (SLA hold
    /// behavior, email settings, colour, etc). Confirmed against the
    /// agent UI's own request.
    pub async fn get_status_details(&self, status_id: i64) -> Result<Value, String> {
        self.get_raw(
            &format!("/api/Status/{status_id}"),
            &[("includedetails", "true".into())],
        )
        .await
    }

    /// Cross-entity search (tickets, clients, users, assets, etc. in one
    /// call). Confirmed against the agent UI's own global search box.
    pub async fn global_search(&self, query: &str, count_per_entity: i64) -> Result<Value, String> {
        self.get_raw(
            "/api/Search",
            &[
                ("search", query.to_string()),
                ("count_per_entity", count_per_entity.to_string()),
            ],
        )
        .await
    }

    /// List custom field groups (collections of custom fields attached to
    /// request forms, e.g. "Laptop Request", "Leaver Details"). Confirmed
    /// against the agent UI's own request.
    pub async fn list_field_groups(&self) -> Result<Vec<Value>, String> {
        let value = self
            .get_raw(
                "/api/FieldGroup",
                &[
                    ("showall", "true".into()),
                    ("access_control_level", "2".into()),
                    ("isconfig", "true".into()),
                ],
            )
            .await?;
        Ok(parse_halo_list::<Value>(value))
    }

    /// List invoices. Confirmed against the agent UI's own request.
    pub async fn list_invoices(&self, page: i64, page_size: i64) -> Result<(Vec<Value>, i64), String> {
        let params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(100).to_string()),
            ("includeinvoices", "true".into()),
            ("includecredits", "false".into()),
            ("includepoinvoices", "false".into()),
        ];
        let value = self.get_raw("/api/Invoice", &params).await?;
        let record_count = value.get("record_count").and_then(|v| v.as_i64()).unwrap_or(0);
        let records = parse_halo_list::<Value>(value);
        Ok((records, record_count))
    }

    /// List recurring invoices. Confirmed against the agent UI's own
    /// request.
    pub async fn list_recurring_invoices(&self, page: i64, page_size: i64) -> Result<(Vec<Value>, i64), String> {
        let params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(100).to_string()),
            ("includeinactive", "true".into()),
            ("includeinvoices", "true".into()),
            ("includecredits", "false".into()),
            ("includepoinvoices", "false".into()),
            ("includelines", "true".into()),
        ];
        let value = self.get_raw("/api/RecurringInvoice", &params).await?;
        let record_count = value.get("record_count").and_then(|v| v.as_i64()).unwrap_or(0);
        let records = parse_halo_list::<Value>(value);
        Ok((records, record_count))
    }

    /// Get a single recurring invoice by ID. Confirmed against the agent
    /// UI's own request.
    pub async fn get_recurring_invoice(&self, recurring_invoice_id: i64) -> Result<Value, String> {
        self.get_raw(
            &format!("/api/RecurringInvoice/{recurring_invoice_id}"),
            &[
                ("includedetails", "true".into()),
                ("includelinkedcreditnotes", "true".into()),
            ],
        )
        .await
    }

    /// List software licences. Endpoint path follows HaloPSA's singular
    /// resource-name convention — NOT confirmed against a real capture
    /// (reverse-engineered from a third-party HaloPSA MCP connector's
    /// output shape against production Halo), flag for retest.
    pub async fn list_software_licences(
        &self,
        page: i64,
        page_size: i64,
        client_id: Option<i64>,
    ) -> Result<(Vec<Value>, i64), String> {
        let mut params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(200).to_string()),
        ];
        if let Some(id) = client_id {
            params.push(("client_id", id.to_string()));
        }
        let value = self.get_raw("/api/Licence", &params).await?;
        let record_count = value.get("record_count").and_then(|v| v.as_i64()).unwrap_or(0);
        let records = parse_halo_list::<Value>(value);
        Ok((records, record_count))
    }

    /// Get a single software licence by ID. Same endpoint-guess caveat as
    /// list_software_licences.
    pub async fn get_software_licence(&self, licence_id: i64) -> Result<Value, String> {
        self.get_raw(
            &format!("/api/Licence/{licence_id}"),
            &[("includedetails", "true".into())],
        )
        .await
    }

    /// List charge types (Configuration > Billing > Charge Types, selected
    /// when billing ticket actions). Confirmed against the agent UI's own
    /// request — a Lookup table (id 17), not a dedicated ChargeRate
    /// endpoint as originally guessed from the StackJack reference.
    pub async fn list_charge_rates(&self) -> Result<Vec<Value>, String> {
        let value = self
            .get_raw(
                "/api/Lookup",
                &[("lookupid", "17".into()), ("showallcodes", "true".into())],
            )
            .await?;
        Ok(parse_halo_list::<Value>(value))
    }

    /// Get HaloPSA instance/system metadata (version, tenant, service
    /// URLs). Endpoint path guessed — NOT confirmed against a real
    /// capture, flag for retest.
    pub async fn get_system_info(&self) -> Result<Value, String> {
        self.get_no_params("/api/SystemInfo").await
    }

    /// List billing lines. HaloPSA has no dedicated billing-lines
    /// endpoint we could confirm — this flattens the `lines` array out of
    /// the invoice list (`/api/Invoice?includelines=true`), the same data
    /// a third-party HaloPSA connector's "billing lines" tool turned out
    /// to be reading from. `includelines` on `/api/Invoice` is itself
    /// unconfirmed, flag for retest.
    pub async fn list_billing_lines(
        &self,
        page: i64,
        page_size: i64,
    ) -> Result<(Vec<Value>, i64), String> {
        let params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(100).to_string()),
            ("includeinvoices", "true".into()),
            ("includecredits", "false".into()),
            ("includepoinvoices", "false".into()),
            ("includelines", "true".into()),
        ];
        let value = self.get_raw("/api/Invoice", &params).await?;
        let record_count = value.get("record_count").and_then(|v| v.as_i64()).unwrap_or(0);
        let mut invoices = parse_halo_list::<Value>(value);
        // /api/Invoice ignores page_size server-side — truncate client-side
        // to bound the number of invoices whose lines we flatten below.
        invoices.truncate(page_size.max(1) as usize);
        let lines: Vec<Value> = invoices
            .iter()
            .flat_map(|inv| {
                inv.get("lines")
                    .and_then(|l| l.as_array())
                    .cloned()
                    .unwrap_or_default()
            })
            .collect();
        Ok((lines, record_count))
    }

    /// Resolve the "Projects" ticket area ID (case-insensitive name
    /// match), used by list_projects/create_project.
    async fn resolve_projects_area_id(&self) -> Result<i64, String> {
        let areas = self.list_ticket_areas().await?;
        areas
            .iter()
            .find(|a| {
                a.get("name")
                    .and_then(|n| n.as_str())
                    .map(|n| n.eq_ignore_ascii_case("projects"))
                    .unwrap_or(false)
            })
            .and_then(|a| a.get("id").and_then(|v| v.as_i64()))
            .ok_or_else(|| "Could not find a ticket area named 'Projects'".to_string())
    }

    /// List projects. Confirmed against the agent UI's own request —
    /// Projects are Tickets scoped to the "Projects" ticket area, not a
    /// separate resource.
    pub async fn list_projects(
        &self,
        page: i64,
        page_size: i64,
        client_id: Option<i64>,
    ) -> Result<(Vec<Value>, i64), String> {
        let area_id = self.resolve_projects_area_id().await?;
        let filters = TicketFilter {
            ticketarea_id: Some(area_id),
            client_id,
            ..Default::default()
        };
        self.list_tickets(page, page_size, &filters).await
    }

    /// Get a single project by ID. Thin wrapper over get_ticket — a
    /// project is just a Ticket scoped to the Projects ticket area.
    pub async fn get_project(&self, project_id: i64) -> Result<Value, String> {
        self.get_ticket(project_id).await
    }

    /// Create a new project. Caller should still supply an appropriate
    /// tickettype_id (use list_ticket_types to find the Project-designated
    /// type) — this wasn't captured from a real "+New Project" form, only
    /// the ticketarea_id is auto-filled and confirmed.
    pub async fn create_project(&self, mut ticket: Value) -> Result<Value, String> {
        let area_id = self.resolve_projects_area_id().await?;
        if let Some(obj) = ticket.as_object_mut() {
            obj.entry("ticketarea_id").or_insert(json!(area_id));
        }
        self.create_ticket(ticket).await
    }

    /// Update an existing project. Thin wrapper over update_ticket.
    pub async fn update_project(&self, project_id: i64, fields: Value) -> Result<Value, String> {
        self.update_ticket(project_id, fields).await
    }

    /// List tasks (child tickets) under a project. Parent/child field name
    /// unconfirmed against a real capture — flag for retest.
    pub async fn list_project_tasks(
        &self,
        project_id: i64,
        page: i64,
        page_size: i64,
    ) -> Result<(Vec<Value>, i64), String> {
        let filters = TicketFilter {
            parent_id: Some(project_id),
            ..Default::default()
        };
        self.list_tickets(page, page_size, &filters).await
    }

    /// Search agents by name/email. Endpoint guessed from list_agents'
    /// confirmed base path — NOT confirmed against a real capture.
    pub async fn search_agents(&self, query: &str, page_size: i64) -> Result<Vec<Value>, String> {
        let params: Vec<(&str, String)> = vec![
            ("search", query.to_string()),
            ("pageinate", "true".into()),
            ("page_no", "1".into()),
            ("page_size", page_size.max(1).min(200).to_string()),
        ];
        let value = self.get_raw("/api/Agent", &params).await?;
        Ok(parse_halo_list::<Value>(value))
    }

    /// List asset types. Endpoint guessed from HaloPSA's singular
    /// resource-name convention — NOT confirmed against a real capture.
    pub async fn list_asset_types(&self) -> Result<Vec<Value>, String> {
        let value = self.get_no_params("/api/AssetType").await?;
        Ok(parse_halo_list::<Value>(value))
    }

    /// List CRM opportunities. Confirmed against the agent UI's own
    /// request — a dedicated /api/Opportunities endpoint (not Tickets,
    /// despite opportunities being stored on the same underlying table).
    pub async fn list_opportunities(
        &self,
        page: i64,
        page_size: i64,
        client_id: Option<i64>,
    ) -> Result<(Vec<Value>, i64), String> {
        let mut params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(200).to_string()),
        ];
        if let Some(id) = client_id {
            params.push(("client_id", id.to_string()));
        }
        let value = self.get_raw("/api/Opportunities", &params).await?;
        let record_count = value.get("record_count").and_then(|v| v.as_i64()).unwrap_or(0);
        let records = parse_halo_list::<Value>(value);
        Ok((records, record_count))
    }

    /// Get a single opportunity by ID. Same base path as the confirmed
    /// list_opportunities.
    pub async fn get_opportunity(&self, opportunity_id: i64) -> Result<Value, String> {
        self.get_raw(
            &format!("/api/Opportunities/{opportunity_id}"),
            &[("includedetails", "true".into())],
        )
        .await
    }

    /// Create a new opportunity. Same base path as the confirmed
    /// list_opportunities; body shape unconfirmed against a real capture.
    pub async fn create_opportunity(&self, opportunity: Value) -> Result<Value, String> {
        self.post("/api/Opportunities", &json!([opportunity])).await
    }

    /// Update an existing opportunity. Same caveats as create_opportunity.
    pub async fn update_opportunity(&self, opportunity_id: i64, mut fields: Value) -> Result<Value, String> {
        if let Some(obj) = fields.as_object_mut() {
            obj.insert("id".into(), json!(opportunity_id));
        }
        self.post("/api/Opportunities", &json!([fields])).await
    }

    /// List CRM notes against a client or supplier. Endpoint confirmed by
    /// name only (a permission-denied response from a third-party
    /// connector's account showed the path is /api/CRMNote) — params and
    /// response shape unconfirmed.
    pub async fn list_crm_notes(&self, client_id: Option<i64>, supplier_id: Option<i64>) -> Result<Vec<Value>, String> {
        let mut params: Vec<(&str, String)> = vec![];
        if let Some(id) = client_id {
            params.push(("client_id", id.to_string()));
        }
        if let Some(id) = supplier_id {
            params.push(("supplier_id", id.to_string()));
        }
        let value = self.get_raw("/api/CRMNote", &params).await?;
        Ok(parse_halo_list::<Value>(value))
    }

    /// Create a CRM note. Same endpoint-confirmed-by-name caveat as
    /// list_crm_notes.
    pub async fn create_crm_note(&self, note: Value) -> Result<Value, String> {
        self.post("/api/CRMNote", &json!([note])).await
    }

    /// List contact groups (shown in the agent UI as "Distribution
    /// Lists"). Confirmed against the agent UI's own request.
    pub async fn list_contact_groups(&self) -> Result<Vec<Value>, String> {
        let value = self
            .get_raw(
                "/api/distributionlists",
                &[
                    ("include_client_related", "false".into()),
                    ("pageinate", "true".into()),
                    ("page_size", "100".into()),
                    ("page_no", "1".into()),
                ],
            )
            .await?;
        Ok(parse_halo_list::<Value>(value))
    }

    /// Add or remove a user from a contact group (distribution list).
    /// Field shape guessed — NOT confirmed against a real capture.
    pub async fn manage_contact_group_members(&self, group_id: i64, user_id: i64, add: bool) -> Result<Value, String> {
        let body = json!({
            "id": group_id,
            "members": [{ "user_id": user_id, "action": if add { "add" } else { "remove" } }],
        });
        self.post("/api/distributionlists", &json!([body])).await
    }

    /// List pending ticket approvals. Confirmed against the agent UI's own
    /// request ("My Approvals" page).
    pub async fn list_ticket_approvals(&self, mine: bool) -> Result<Vec<Value>, String> {
        let value = self
            .get_raw(
                "/api/TicketApproval",
                &[("mine", mine.to_string()), ("include_attachments", "false".into())],
            )
            .await?;
        Ok(parse_halo_list::<Value>(value))
    }

    /// Approve or reject one or more pending approvals. Body shape
    /// guessed — NOT confirmed against a real capture. Irreversible once
    /// processed, per HaloPSA's own approval workflow semantics.
    pub async fn process_approval(&self, approval_ids: &[i64], approve: bool) -> Result<Value, String> {
        let body = json!({
            "ids": approval_ids,
            "result": if approve { 1 } else { 2 },
        });
        self.post("/api/TicketApproval", &body).await
    }

    /// List CSAT feedback entries. Endpoint guessed from HaloPSA's
    /// singular resource-name convention — NOT confirmed against a real
    /// capture.
    pub async fn list_feedback(&self, client_id: Option<i64>, agent_id: Option<i64>) -> Result<Vec<Value>, String> {
        let mut params: Vec<(&str, String)> = vec![];
        if let Some(id) = client_id {
            params.push(("client_id", id.to_string()));
        }
        if let Some(id) = agent_id {
            params.push(("agent_id", id.to_string()));
        }
        let value = self.get_raw("/api/Feedback", &params).await?;
        Ok(parse_halo_list::<Value>(value))
    }

    /// List custom data tables. Endpoint guessed from HaloPSA's naming
    /// convention (matches the shape returned by a third-party HaloPSA
    /// connector) — NOT confirmed against a real capture.
    pub async fn list_custom_tables(&self) -> Result<Vec<Value>, String> {
        let value = self.get_no_params("/api/CustomTable").await?;
        Ok(parse_halo_list::<Value>(value))
    }

    /// Get a single custom table by ID. Same endpoint-guess caveat as
    /// list_custom_tables.
    pub async fn get_custom_table(&self, table_id: i64) -> Result<Value, String> {
        self.get_raw(
            &format!("/api/CustomTable/{table_id}"),
            &[("includedetails", "true".into())],
        )
        .await
    }

    /// Create a custom data table. Same endpoint-guess caveat as
    /// list_custom_tables.
    pub async fn create_custom_table(&self, table: Value) -> Result<Value, String> {
        self.post("/api/CustomTable", &json!([table])).await
    }

    /// Delete a custom data table by ID. Same endpoint-guess caveat as
    /// list_custom_tables. Permanently removes the table and its data.
    pub async fn delete_custom_table(&self, table_id: i64) -> Result<(), String> {
        self.delete(&format!("/api/CustomTable/{table_id}"), &[]).await
    }

    /// Update an existing client's fields.
    pub async fn update_client(&self, client_id: i64, mut fields: Value) -> Result<Value, String> {
        if let Some(obj) = fields.as_object_mut() {
            obj.insert("id".into(), json!(client_id));
        }
        self.post("/api/Client", &json!([fields])).await
    }

    /// Generate a PDF from a saved report. Params extend the confirmed
    /// run_report call with an ispdf flag — NOT confirmed against a real
    /// capture.
    pub async fn create_report_pdf(&self, report_id: i64, filters: &[Value]) -> Result<Value, String> {
        let mut params: Vec<(&str, String)> = vec![
            ("includedetails", "true".into()),
            ("loadreport", "true".into()),
            ("dontloadsystemreport", "false".into()),
            ("ispdf", "true".into()),
        ];
        if !filters.is_empty() {
            params.push(("filters", json!(filters).to_string()));
        }
        self.get_raw(&format!("/api/Report/{report_id}"), &params).await
    }

    // --- Assets (audit trail / licence assignment). Endpoints guessed;
    // list_device_licences reuses the same base path as the confirmed
    // list_software_licences with a different filter ---

    pub async fn list_asset_changes(&self, asset_id: Option<i64>, count: i64) -> Result<Vec<Value>, String> {
        let mut params: Vec<(&str, String)> = vec![("count", count.max(1).min(200).to_string())];
        if let Some(id) = asset_id {
            params.push(("asset_id", id.to_string()));
        }
        let value = self.get_raw("/api/AssetChange", &params).await?;
        Ok(parse_halo_list::<Value>(value))
    }

    pub async fn list_device_licences(&self, device_id: Option<i64>) -> Result<Vec<Value>, String> {
        let mut params: Vec<(&str, String)> = vec![("pageinate", "true".into()), ("page_size", "100".into())];
        if let Some(id) = device_id {
            params.push(("asset_id", id.to_string()));
        }
        let value = self.get_raw("/api/Licence", &params).await?;
        Ok(parse_halo_list::<Value>(value))
    }

    // --- Invoices (create/void). create_invoice reuses the confirmed
    // /api/Invoice base path; void_invoice's field name is guessed ---

    pub async fn create_invoice(&self, invoice: Value) -> Result<Value, String> {
        self.post("/api/Invoice", &json!([invoice])).await
    }

    pub async fn void_invoice(&self, invoice_id: i64) -> Result<Value, String> {
        self.post("/api/Invoice", &json!([{ "id": invoice_id, "voidinvoice": true }])).await
    }

    // --- Suppliers (endpoints guessed from HaloPSA's naming convention.
    // create_supplier_user's payload shape is informed by a third-party
    // connector's own documented behavior: POST to /api/Users with
    // isuserdetails=true and an embedded supplier object, since suppliers
    // are Users under the hood, same as clients) ---

    pub async fn list_suppliers(&self, count: i64) -> Result<Vec<Value>, String> {
        let value = self
            .get_raw("/api/Supplier", &[("count", count.max(1).min(200).to_string())])
            .await?;
        Ok(parse_halo_list::<Value>(value))
    }

    pub async fn get_supplier(&self, supplier_id: i64) -> Result<Value, String> {
        self.get_raw(&format!("/api/Supplier/{supplier_id}"), &[("includedetails", "true".into())])
            .await
    }

    pub async fn create_supplier(&self, supplier: Value) -> Result<Value, String> {
        self.post("/api/Supplier", &json!([supplier])).await
    }

    pub async fn create_supplier_user(&self, supplier_id: i64, supplier_name: &str, mut fields: Value) -> Result<Value, String> {
        if let Some(obj) = fields.as_object_mut() {
            obj.insert("isuserdetails".into(), json!(true));
            obj.insert("supplier".into(), json!({ "id": supplier_id, "lookupdisplay": supplier_name }));
        }
        self.post("/api/Users", &json!([fields])).await
    }

    // --- Sales Orders (endpoints guessed from HaloPSA's naming
    // convention; NOT confirmed against a real capture) ---

    pub async fn list_sales_orders(&self, client_id: Option<i64>, page: i64, page_size: i64) -> Result<Vec<Value>, String> {
        let mut params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(100).to_string()),
        ];
        if let Some(id) = client_id {
            params.push(("client_id", id.to_string()));
        }
        let value = self.get_raw("/api/SalesOrder", &params).await?;
        Ok(parse_halo_list::<Value>(value))
    }

    pub async fn get_sales_order(&self, sales_order_id: i64) -> Result<Value, String> {
        self.get_raw(
            &format!("/api/SalesOrder/{sales_order_id}"),
            &[("includedetails", "true".into())],
        )
        .await
    }

    pub async fn create_sales_order(&self, sales_order: Value) -> Result<Value, String> {
        self.post("/api/SalesOrder", &json!([sales_order])).await
    }

    // --- Audit (endpoints guessed from HaloPSA's naming convention) ---

    pub async fn get_audit_entry(&self, audit_entry_id: i64) -> Result<Value, String> {
        self.get_raw(&format!("/api/AuditEntry/{audit_entry_id}"), &[]).await
    }

    pub async fn get_field(&self, field_id: i64) -> Result<Value, String> {
        self.get_raw(&format!("/api/Field/{field_id}"), &[]).await
    }

    // --- Integration Data (remaining passthroughs) / Integration Sync ---

    pub async fn get_pax8_data(&self, datatype: Option<&str>, search: Option<&str>) -> Result<Value, String> {
        self.get_integration_data("Pax8", datatype, search).await
    }

    pub async fn get_meraki_data(&self) -> Result<Value, String> {
        self.get_integration_data("Meraki", None, None).await
    }

    /// Trigger a data import sync against the connected Xero integration.
    /// Endpoint guessed — irreversibly syncs external data, never invoked
    /// here.
    pub async fn import_from_xero(&self, options: Value) -> Result<Value, String> {
        self.post("/api/Xero/Import", &options).await
    }

    // --- Approval Processes (endpoints guessed from HaloPSA's naming
    // convention; NOT confirmed against a real capture) ---

    pub async fn list_approval_processes(&self) -> Result<Vec<Value>, String> {
        let value = self.get_no_params("/api/ApprovalProcess").await?;
        Ok(parse_halo_list::<Value>(value))
    }

    pub async fn get_approval_process(&self, process_id: i64) -> Result<Value, String> {
        self.get_raw(
            &format!("/api/ApprovalProcess/{process_id}"),
            &[("includedetails", "true".into())],
        )
        .await
    }

    pub async fn create_approval_process(&self, process: Value) -> Result<Value, String> {
        self.post("/api/ApprovalProcess", &json!([process])).await
    }

    pub async fn list_approval_rules(&self, process_id: Option<i64>) -> Result<Vec<Value>, String> {
        let mut params: Vec<(&str, String)> = vec![];
        if let Some(id) = process_id {
            params.push(("process_id", id.to_string()));
        }
        let value = self.get_raw("/api/ApprovalRule", &params).await?;
        Ok(parse_halo_list::<Value>(value))
    }

    pub async fn create_approval_rule(&self, rule: Value) -> Result<Value, String> {
        self.post("/api/ApprovalRule", &json!([rule])).await
    }

    // --- Saved Views. list_views and list_view_filters reuse endpoints
    // confirmed via the agent UI's own requests during the Projects/
    // Opportunities investigation (/api/viewlists, /api/ViewFilter) ---

    pub async fn list_views(&self, domain: &str, view_type: &str) -> Result<Vec<Value>, String> {
        let value = self
            .get_raw(
                "/api/viewlists",
                &[
                    ("showcounts", "true".into()),
                    ("domain", domain.into()),
                    ("type", view_type.into()),
                ],
            )
            .await?;
        Ok(parse_halo_list::<Value>(value))
    }

    /// Get a single saved view by ID. Endpoint guessed — NOT confirmed
    /// against a real capture (list_views itself is confirmed).
    pub async fn get_view(&self, view_id: i64) -> Result<Value, String> {
        self.get_raw(&format!("/api/View/{view_id}"), &[]).await
    }

    pub async fn list_view_filters(&self, view_type: &str, ticketarea_id: Option<i64>) -> Result<Vec<Value>, String> {
        let mut params: Vec<(&str, String)> = vec![("type", view_type.into())];
        if let Some(id) = ticketarea_id {
            params.push(("ticketarea_id", id.to_string()));
        }
        let value = self.get_raw("/api/ViewFilter", &params).await?;
        Ok(parse_halo_list::<Value>(value))
    }

    /// List available columns for saved views. Endpoint guessed — NOT
    /// confirmed against a real capture (list_view_filters itself is
    /// confirmed).
    pub async fn list_view_columns(&self, domain: Option<&str>) -> Result<Vec<Value>, String> {
        let mut params: Vec<(&str, String)> = vec![];
        if let Some(d) = domain {
            params.push(("domain", d.to_string()));
        }
        let value = self.get_raw("/api/ViewColumn", &params).await?;
        Ok(parse_halo_list::<Value>(value))
    }

    /// Create a new client. Reuses the same confirmed /api/Client base
    /// path as get_client/list_clients.
    pub async fn create_client(&self, client_data: Value) -> Result<Value, String> {
        self.post("/api/Client", &json!([client_data])).await
    }

    /// List payments recorded against invoices. Endpoint guessed from
    /// HaloPSA's naming convention — NOT confirmed against a real capture.
    pub async fn list_invoice_payments(&self, invoice_id: Option<i64>, count: i64) -> Result<Vec<Value>, String> {
        let mut params: Vec<(&str, String)> = vec![("count", count.max(1).min(200).to_string())];
        if let Some(id) = invoice_id {
            params.push(("invoice_id", id.to_string()));
        }
        let value = self.get_raw("/api/InvoicePayment", &params).await?;
        Ok(parse_halo_list::<Value>(value))
    }

    // --- Workflows (list/get reuse the already-confirmed /api/Workflow
    // base path used internally by the existing get_workflow/
    // list_workflow_steps; write operations are unconfirmed) ---

    pub async fn list_workflows(&self, page: i64, page_size: i64) -> Result<Vec<Value>, String> {
        let params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(100).to_string()),
        ];
        let value = self.get_raw("/api/Workflow", &params).await?;
        let mut records = parse_halo_list::<Value>(value);
        // /api/Workflow ignores page_size server-side — truncate client-side.
        records.truncate(page_size.max(1) as usize);
        Ok(records)
    }

    /// Get full raw workflow details by ID. Same confirmed endpoint as the
    /// existing get_workflow (used internally for steps), but returns the
    /// full JSON rather than the narrower typed Workflow struct.
    pub async fn get_workflow_details(&self, workflow_id: i64) -> Result<Value, String> {
        self.get_raw(
            &format!("/api/Workflow/{workflow_id}"),
            &[("includedetails", "true".into())],
        )
        .await
    }

    pub async fn create_workflow(&self, workflow: Value) -> Result<Value, String> {
        self.post("/api/Workflow", &json!([workflow])).await
    }

    pub async fn update_workflow(&self, workflow_id: i64, mut fields: Value) -> Result<Value, String> {
        if let Some(obj) = fields.as_object_mut() {
            obj.insert("id".into(), json!(workflow_id));
        }
        self.post("/api/Workflow", &json!([fields])).await
    }

    pub async fn delete_workflow(&self, workflow_id: i64) -> Result<(), String> {
        self.delete(&format!("/api/Workflow/{workflow_id}"), &[]).await
    }

    // --- Notifications (endpoints guessed from HaloPSA's naming
    // convention; NOT confirmed against a real capture) ---

    pub async fn list_notifications(
        &self,
        agent_id: Option<i64>,
        page: i64,
        page_size: i64,
    ) -> Result<Vec<Value>, String> {
        let mut params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(100).to_string()),
        ];
        if let Some(id) = agent_id {
            params.push(("agent_id", id.to_string()));
        }
        let value = self.get_raw("/api/Notification", &params).await?;
        let mut records = parse_halo_list::<Value>(value);
        // /api/Notification ignores page_size server-side — truncate client-side.
        records.truncate(page_size.max(1) as usize);
        Ok(records)
    }

    pub async fn get_notification(&self, notification_id: i64) -> Result<Value, String> {
        self.get_raw(&format!("/api/Notification/{notification_id}"), &[]).await
    }

    pub async fn create_notification(&self, notification: Value) -> Result<Value, String> {
        self.post("/api/Notification", &json!([notification])).await
    }

    pub async fn send_notification_message(&self, message: Value) -> Result<Value, String> {
        self.post("/api/NotificationMessage", &json!([message])).await
    }

    // --- Service Catalog (endpoints guessed from HaloPSA's naming
    // convention; NOT confirmed against a real capture) ---

    pub async fn list_services(
        &self,
        search: Option<&str>,
        category_id: Option<i64>,
        page: i64,
        page_size: i64,
    ) -> Result<(Vec<Value>, i64), String> {
        let mut params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(100).to_string()),
        ];
        if let Some(s) = search {
            params.push(("search", s.to_string()));
        }
        if let Some(id) = category_id {
            params.push(("service_category_id", id.to_string()));
        }
        let value = self.get_raw("/api/Service", &params).await?;
        let record_count = value.get("record_count").and_then(|v| v.as_i64()).unwrap_or(0);
        let records = parse_halo_list::<Value>(value);
        Ok((records, record_count))
    }

    pub async fn get_service(&self, service_id: i64) -> Result<Value, String> {
        self.get_raw(
            &format!("/api/Service/{service_id}"),
            &[("includedetails", "true".into())],
        )
        .await
    }

    pub async fn list_service_categories(&self) -> Result<Vec<Value>, String> {
        let value = self.get_no_params("/api/ServiceCategory").await?;
        Ok(parse_halo_list::<Value>(value))
    }

    pub async fn list_service_statuses(&self, service_id: Option<i64>) -> Result<Vec<Value>, String> {
        let mut params: Vec<(&str, String)> = vec![];
        if let Some(id) = service_id {
            params.push(("service_id", id.to_string()));
        }
        let value = self.get_raw("/api/ServiceStatus", &params).await?;
        Ok(parse_halo_list::<Value>(value))
    }

    pub async fn create_service_status(&self, status: Value) -> Result<Value, String> {
        self.post("/api/ServiceStatus", &json!([status])).await
    }

    // --- Invoice & Order Extras (endpoints guessed from HaloPSA's naming
    // convention; NOT confirmed against a real capture. review_expense and
    // expire_client_prepay are financial actions with real-world
    // consequences — implemented but never invoked here) ---

    pub async fn list_invoice_statuses(&self) -> Result<Vec<Value>, String> {
        let value = self.get_no_params("/api/InvoiceStatus").await?;
        Ok(parse_halo_list::<Value>(value))
    }

    pub async fn get_invoice_status(&self, status_id: i64) -> Result<Value, String> {
        self.get_raw(&format!("/api/InvoiceStatus/{status_id}"), &[]).await
    }

    pub async fn create_invoice_status(&self, status: Value) -> Result<Value, String> {
        self.post("/api/InvoiceStatus", &json!([status])).await
    }

    pub async fn delete_invoice_status(&self, status_id: i64) -> Result<(), String> {
        self.delete(&format!("/api/InvoiceStatus/{status_id}"), &[]).await
    }

    pub async fn update_invoice_lines(&self, lines: Value) -> Result<Value, String> {
        self.post("/api/InvoiceLine", &lines).await
    }

    pub async fn update_sales_order_lines(&self, lines: Value) -> Result<Value, String> {
        self.post("/api/SalesOrderLine", &lines).await
    }

    pub async fn register_invoice_view(&self, invoice_id: i64) -> Result<Value, String> {
        self.post("/api/InvoiceView", &json!({ "invoice_id": invoice_id })).await
    }

    pub async fn register_sales_order_view(&self, sales_order_id: i64) -> Result<Value, String> {
        self.post("/api/SalesOrderView", &json!({ "salesorder_id": sales_order_id }))
            .await
    }

    pub async fn register_purchase_order_view(&self, purchase_order_id: i64) -> Result<Value, String> {
        self.post(
            "/api/PurchaseOrderView",
            &json!({ "purchaseorder_id": purchase_order_id }),
        )
        .await
    }

    pub async fn register_kb_article_view(&self, kb_article_id: i64) -> Result<Value, String> {
        self.post("/api/KBArticleView", &json!({ "kbarticle_id": kb_article_id }))
            .await
    }

    pub async fn review_expense(&self, expense_ids: &[i64]) -> Result<Value, String> {
        let body: Vec<Value> = expense_ids
            .iter()
            .map(|id| json!({ "id": id, "reviewed": true }))
            .collect();
        self.post("/api/Expense", &json!(body)).await
    }

    pub async fn expire_client_prepay(&self, prepay_ids: &[i64]) -> Result<Value, String> {
        let body: Vec<Value> = prepay_ids
            .iter()
            .map(|id| json!({ "id": id, "expired": true }))
            .collect();
        self.post("/api/ClientPrepay", &json!(body)).await
    }

    // --- Integration Management (endpoints guessed from HaloPSA's naming
    // convention; real field shapes for list_integration_configs
    // confirmed via StackJack reference, endpoint path itself unconfirmed) ---

    pub async fn list_integration_configs(&self) -> Result<Vec<Value>, String> {
        let value = self.get_no_params("/api/Integration").await?;
        Ok(parse_halo_list::<Value>(value))
    }

    pub async fn get_integration_config(&self, config_id: i64) -> Result<Value, String> {
        self.get_raw(
            &format!("/api/Integration/{config_id}"),
            &[("includedetails", "true".into())],
        )
        .await
    }

    pub async fn list_integration_site_mappings(&self, module_id: Option<i64>) -> Result<Vec<Value>, String> {
        let mut params: Vec<(&str, String)> = vec![];
        if let Some(id) = module_id {
            params.push(("module_id", id.to_string()));
        }
        let value = self.get_raw("/api/IntegrationSiteMapping", &params).await?;
        Ok(parse_halo_list::<Value>(value))
    }

    pub async fn list_integration_errors(
        &self,
        module_id: Option<i64>,
        page: i64,
        page_size: i64,
    ) -> Result<Vec<Value>, String> {
        let mut params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(200).to_string()),
        ];
        if let Some(id) = module_id {
            params.push(("module_id", id.to_string()));
        }
        let value = self.get_raw("/api/IntegrationError", &params).await?;
        Ok(parse_halo_list::<Value>(value))
    }

    pub async fn get_integration_error(&self, error_id: i64) -> Result<Value, String> {
        self.get_raw(&format!("/api/IntegrationError/{error_id}"), &[]).await
    }

    pub async fn list_integration_requests(
        &self,
        module_id: Option<i64>,
        page: i64,
        page_size: i64,
    ) -> Result<Vec<Value>, String> {
        let mut params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(200).to_string()),
        ];
        if let Some(id) = module_id {
            params.push(("module_id", id.to_string()));
        }
        let value = self.get_raw("/api/IntegrationRequest", &params).await?;
        Ok(parse_halo_list::<Value>(value))
    }

    pub async fn get_integration_request(&self, request_id: i64) -> Result<Value, String> {
        self.get_raw(&format!("/api/IntegrationRequest/{request_id}"), &[]).await
    }

    pub async fn list_integration_field_mappings(&self, module_id: Option<i64>) -> Result<Vec<Value>, String> {
        let mut params: Vec<(&str, String)> = vec![];
        if let Some(id) = module_id {
            params.push(("module_id", id.to_string()));
        }
        let value = self.get_raw("/api/IntegrationFieldMapping", &params).await?;
        Ok(parse_halo_list::<Value>(value))
    }

    // --- Integration Data (third-party passthrough proxies — highest
    // uncertainty in this batch: endpoint guessed, AND depends on which
    // integrations are actually configured/authorized on this tenant.
    // list_integration_configs can be used to check connection status
    // first) ---

    async fn get_integration_data(
        &self,
        system: &str,
        datatype: Option<&str>,
        search: Option<&str>,
    ) -> Result<Value, String> {
        let mut params: Vec<(&str, String)> = vec![];
        if let Some(dt) = datatype {
            params.push(("datatype", dt.to_string()));
        }
        if let Some(s) = search {
            params.push(("search", s.to_string()));
        }
        // A third-party HaloPSA connector's own tool descriptions for
        // SentinelOne/Sophos noted the legacy path is
        // /api/IntegrationData/Get/{System} — some integrations may have
        // moved off this pattern since, so this is a better-informed guess
        // than a flat /api/{System}, not a confirmed capture.
        self.get_raw(&format!("/api/IntegrationData/Get/{system}"), &params).await
    }

    pub async fn get_microsoft_csp_data(&self, datatype: Option<&str>, search: Option<&str>) -> Result<Value, String> {
        self.get_integration_data("MicrosoftCSP", datatype, search).await
    }

    pub async fn get_intune_data(&self, datatype: Option<&str>, search: Option<&str>) -> Result<Value, String> {
        self.get_integration_data("Intune", datatype, search).await
    }

    pub async fn get_azure_ad_data(&self, datatype: Option<&str>, search: Option<&str>) -> Result<Value, String> {
        self.get_integration_data("AzureAD", datatype, search).await
    }

    pub async fn get_ninja_rmm_data(&self) -> Result<Value, String> {
        self.get_no_params("/api/NinjaRMM").await
    }

    pub async fn get_xero_data(&self, datatype: Option<&str>, search: Option<&str>) -> Result<Value, String> {
        self.get_integration_data("Xero", datatype, search).await
    }

    // --- Accounting Details (endpoint guessed; NOT confirmed) ---

    pub async fn list_xero_details(&self, page: i64, page_size: i64) -> Result<Vec<Value>, String> {
        let params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(100).to_string()),
        ];
        let value = self.get_raw("/api/XeroDetail", &params).await?;
        Ok(parse_halo_list::<Value>(value))
    }

    pub async fn get_xero_detail(&self, detail_id: i64) -> Result<Value, String> {
        self.get_raw(&format!("/api/XeroDetail/{detail_id}"), &[]).await
    }

    // --- Integration Sync (endpoint guessed; NOT confirmed) ---

    pub async fn send_invoice_to_xero(&self, invoice_id: i64) -> Result<Value, String> {
        self.post("/api/Xero/SendInvoice", &json!({ "invoice_id": invoice_id }))
            .await
    }

    // --- Assets (remaining write operations + software inventory). Same
    // confirmed /api/Asset base path as the existing get_asset/list_assets
    // for create/update; list_asset_software endpoint itself guessed ---

    pub async fn create_asset(&self, asset: Value) -> Result<Value, String> {
        self.post("/api/Asset", &json!([asset])).await
    }

    pub async fn update_asset(&self, asset_id: i64, mut fields: Value) -> Result<Value, String> {
        if let Some(obj) = fields.as_object_mut() {
            obj.insert("id".into(), json!(asset_id));
        }
        self.post("/api/Asset", &json!([fields])).await
    }

    pub async fn list_asset_software(&self, asset_id: Option<i64>) -> Result<Vec<Value>, String> {
        let mut params: Vec<(&str, String)> = vec![];
        if let Some(id) = asset_id {
            params.push(("device_id", id.to_string()));
        }
        let value = self.get_raw("/api/AssetSoftware", &params).await?;
        Ok(parse_halo_list::<Value>(value))
    }

    // --- Roles (endpoints guessed from HaloPSA's naming convention; NOT
    // confirmed against a real capture, flag for retest) ---

    pub async fn list_roles(&self, page: i64, page_size: i64) -> Result<(Vec<Value>, i64), String> {
        let params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(100).to_string()),
        ];
        let value = self.get_raw("/api/Role", &params).await?;
        let record_count = value.get("record_count").and_then(|v| v.as_i64()).unwrap_or(0);
        let records = parse_halo_list::<Value>(value);
        Ok((records, record_count))
    }

    pub async fn get_role(&self, role_id: i64) -> Result<Value, String> {
        self.get_raw(&format!("/api/Role/{role_id}"), &[("includedetails", "true".into())]).await
    }

    pub async fn create_role(&self, role: Value) -> Result<Value, String> {
        self.post("/api/Role", &json!([role])).await
    }

    pub async fn update_role(&self, role_id: i64, mut fields: Value) -> Result<Value, String> {
        if let Some(obj) = fields.as_object_mut() {
            obj.insert("id".into(), json!(role_id));
        }
        self.post("/api/Role", &json!([fields])).await
    }

    pub async fn delete_role(&self, role_id: i64) -> Result<(), String> {
        self.delete(&format!("/api/Role/{role_id}"), &[]).await
    }

    // --- Tags (endpoint name confirmed by a permission-denied response
    // from a third-party connector's account: /api/Tags. Params/shape
    // unconfirmed) ---

    pub async fn list_tags(&self, search: Option<&str>, page: i64, page_size: i64) -> Result<Vec<Value>, String> {
        let mut params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(100).to_string()),
        ];
        if let Some(s) = search {
            params.push(("search", s.to_string()));
        }
        let value = self.get_raw("/api/Tags", &params).await?;
        let mut records = parse_halo_list::<Value>(value);
        // /api/Tags ignores page_size server-side — truncate client-side.
        records.truncate(page_size.max(1) as usize);
        Ok(records)
    }

    pub async fn get_tag(&self, tag_id: i64) -> Result<Value, String> {
        self.get_raw(&format!("/api/Tags/{tag_id}"), &[]).await
    }

    pub async fn create_tag(&self, tag: Value) -> Result<Value, String> {
        self.post("/api/Tags", &json!([tag])).await
    }

    pub async fn delete_tag(&self, tag_id: i64) -> Result<(), String> {
        self.delete(&format!("/api/Tags/{tag_id}"), &[]).await
    }

    // --- Item Groups / Items / Item Stock (endpoints guessed from
    // HaloPSA's naming convention; NOT confirmed against a real capture) ---

    pub async fn list_item_groups(&self, page: i64, page_size: i64) -> Result<Vec<Value>, String> {
        let params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(200).to_string()),
        ];
        let value = self.get_raw("/api/ItemGroup", &params).await?;
        let mut records = parse_halo_list::<Value>(value);
        // /api/ItemGroup ignores page_size server-side — truncate client-side.
        records.truncate(page_size.max(1) as usize);
        Ok(records)
    }

    pub async fn get_item_group(&self, group_id: i64) -> Result<Value, String> {
        self.get_raw(&format!("/api/ItemGroup/{group_id}"), &[]).await
    }

    pub async fn create_item_group(&self, group: Value) -> Result<Value, String> {
        self.post("/api/ItemGroup", &json!([group])).await
    }

    pub async fn delete_item_group(&self, group_id: i64) -> Result<(), String> {
        self.delete(&format!("/api/ItemGroup/{group_id}"), &[]).await
    }

    pub async fn list_items(&self, search: Option<&str>, page: i64, page_size: i64) -> Result<(Vec<Value>, i64), String> {
        let mut params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(200).to_string()),
        ];
        if let Some(s) = search {
            params.push(("search", s.to_string()));
        }
        let value = self.get_raw("/api/Item", &params).await?;
        let record_count = value.get("record_count").and_then(|v| v.as_i64()).unwrap_or(0);
        let records = parse_halo_list::<Value>(value);
        Ok((records, record_count))
    }

    pub async fn get_item(&self, item_id: i64) -> Result<Value, String> {
        self.get_raw(&format!("/api/Item/{item_id}"), &[("includedetails", "true".into())]).await
    }

    pub async fn create_item(&self, item: Value) -> Result<Value, String> {
        self.post("/api/Item", &json!([item])).await
    }

    pub async fn list_item_stock(&self, item_id: Option<i64>) -> Result<Vec<Value>, String> {
        let mut params: Vec<(&str, String)> = vec![];
        if let Some(id) = item_id {
            params.push(("item_id", id.to_string()));
        }
        let value = self.get_raw("/api/ItemStock", &params).await?;
        Ok(parse_halo_list::<Value>(value))
    }

    // --- Products & Components (endpoints guessed from HaloPSA's naming
    // convention; NOT confirmed against a real capture) ---

    pub async fn list_products(&self, search: Option<&str>, page: i64, page_size: i64) -> Result<Vec<Value>, String> {
        let mut params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(100).to_string()),
        ];
        if let Some(s) = search {
            params.push(("search", s.to_string()));
        }
        let value = self.get_raw("/api/Product", &params).await?;
        Ok(parse_halo_list::<Value>(value))
    }

    pub async fn get_product(&self, product_id: i64) -> Result<Value, String> {
        self.get_raw(&format!("/api/Product/{product_id}"), &[("includedetails", "true".into())]).await
    }

    pub async fn create_product(&self, product: Value) -> Result<Value, String> {
        self.post("/api/Product", &json!([product])).await
    }

    pub async fn delete_product(&self, product_id: i64) -> Result<(), String> {
        self.delete(&format!("/api/Product/{product_id}"), &[]).await
    }

    pub async fn list_product_components(
        &self,
        product_id: Option<i64>,
        page: i64,
        page_size: i64,
    ) -> Result<Vec<Value>, String> {
        let mut params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(100).to_string()),
        ];
        if let Some(id) = product_id {
            params.push(("product_id", id.to_string()));
        }
        let value = self.get_raw("/api/ProductComponent", &params).await?;
        Ok(parse_halo_list::<Value>(value))
    }

    pub async fn create_product_component(&self, component: Value) -> Result<Value, String> {
        self.post("/api/ProductComponent", &json!([component])).await
    }

    // --- Quotations (endpoints guessed from HaloPSA's naming convention;
    // NOT confirmed against a real capture) ---

    pub async fn list_quotations(
        &self,
        client_id: Option<i64>,
        page: i64,
        page_size: i64,
    ) -> Result<(Vec<Value>, i64), String> {
        let mut params: Vec<(&str, String)> = vec![
            ("pageinate", "true".into()),
            ("page_no", page.to_string()),
            ("page_size", page_size.max(1).min(200).to_string()),
        ];
        if let Some(id) = client_id {
            params.push(("client_id", id.to_string()));
        }
        let value = self.get_raw("/api/Quotation", &params).await?;
        let record_count = value.get("record_count").and_then(|v| v.as_i64()).unwrap_or(0);
        let records = parse_halo_list::<Value>(value);
        Ok((records, record_count))
    }

    pub async fn get_quotation(&self, quotation_id: i64) -> Result<Value, String> {
        self.get_raw(
            &format!("/api/Quotation/{quotation_id}"),
            &[("includedetails", "true".into())],
        )
        .await
    }

    pub async fn create_quotation(&self, quotation: Value) -> Result<Value, String> {
        self.post("/api/Quotation", &json!([quotation])).await
    }

    pub async fn update_quotation_lines(&self, quotation_id: i64, lines: Value) -> Result<Value, String> {
        let body = json!({ "id": quotation_id, "lines": lines });
        self.post("/api/Quotation", &json!([body])).await
    }

    pub async fn approve_quotation(&self, quotation_id: i64, approved: bool, notes: Option<&str>) -> Result<Value, String> {
        let mut body = json!({ "id": quotation_id, "approvalstate": if approved { 2 } else { 3 } });
        if let Some(n) = notes {
            if let Some(obj) = body.as_object_mut() {
                obj.insert("approvalnote".into(), json!(n));
            }
        }
        self.post("/api/Quotation", &json!([body])).await
    }

    /// View a quotation in presentation form. No distinct "rendered view"
    /// endpoint was found — thin wrapper over get_quotation.
    pub async fn view_quotation(&self, quotation_id: i64) -> Result<Value, String> {
        self.get_quotation(quotation_id).await
    }

    // --- Timesheets (endpoint guessed from HaloPSA's naming convention;
    // NOT confirmed against a real capture. StackJack's reference shape
    // for this area looked like a per-day target-vs-actual rollup rather
    // than discrete loggable entries, so get_timesheet/create_timesheet
    // are higher-risk guesses than list_timesheets/get_my_timesheets) ---

    pub async fn list_timesheets(&self, agent_id: Option<i64>) -> Result<Vec<Value>, String> {
        let mut params: Vec<(&str, String)> = vec![];
        if let Some(id) = agent_id {
            params.push(("agent_id", id.to_string()));
        }
        let value = self.get_raw("/api/Timesheet", &params).await?;
        Ok(parse_halo_list::<Value>(value))
    }

    /// List the current agent's timesheets. The guessed `mine=true` query
    /// param is silently ignored by this endpoint (confirmed: returned the
    /// same unfiltered dataset as list_timesheets with no agent_id) — use
    /// the confirmed-working agent_id filter instead, resolved via get_me.
    pub async fn get_my_timesheets(&self) -> Result<Vec<Value>, String> {
        let me = self.get_me().await?;
        let agent_id = me
            .get("agentid")
            .or_else(|| me.get("agent_id"))
            .or_else(|| me.get("id"))
            .and_then(|v| v.as_i64())
            .ok_or("Could not resolve your agent ID from get_me")?;
        self.list_timesheets(Some(agent_id)).await
    }

    pub async fn get_timesheet(&self, timesheet_id: i64) -> Result<Value, String> {
        self.get_raw(&format!("/api/Timesheet/{timesheet_id}"), &[]).await
    }

    pub async fn create_timesheet(&self, entry: Value) -> Result<Value, String> {
        self.post("/api/Timesheet", &json!([entry])).await
    }

    // --- Invoices (remaining items) ---

    /// Get a single invoice by ID. Same base path as the confirmed
    /// list_invoices.
    pub async fn get_invoice(&self, invoice_id: i64) -> Result<Value, String> {
        self.get_raw(
            &format!("/api/Invoice/{invoice_id}"),
            &[("includedetails", "true".into())],
        )
        .await
    }

    /// List invoice line items — for a specific invoice if given, else
    /// falls back to the same flattened-across-recent-invoices approach as
    /// list_billing_lines.
    pub async fn list_invoice_lines(&self, invoice_id: Option<i64>) -> Result<Vec<Value>, String> {
        if let Some(id) = invoice_id {
            let invoice = self.get_invoice(id).await?;
            return Ok(invoice
                .get("lines")
                .and_then(|l| l.as_array())
                .cloned()
                .unwrap_or_default());
        }
        let (lines, _) = self.list_billing_lines(1, 50).await?;
        Ok(lines)
    }

    // --- Asset Groups (remaining write operations). Same endpoint-guess
    // caveat as the existing list_asset_groups/get_asset_group ---

    pub async fn create_asset_group(&self, group: Value) -> Result<Value, String> {
        self.post("/api/AssetGroup", &json!([group])).await
    }

    pub async fn update_asset_group(&self, group_id: i64, mut fields: Value) -> Result<Value, String> {
        if let Some(obj) = fields.as_object_mut() {
            obj.insert("id".into(), json!(group_id));
        }
        self.post("/api/AssetGroup", &json!([fields])).await
    }

    pub async fn delete_asset_group(&self, group_id: i64) -> Result<(), String> {
        self.delete(&format!("/api/AssetGroup/{group_id}"), &[]).await
    }

    // --- Attachments (metadata only — upload/download/binary variants
    // deferred, they need blob/base64 handling rather than a plain JSON
    // call. Endpoint guessed from HaloPSA's naming convention) ---

    pub async fn list_attachments(&self, ticket_id: Option<i64>) -> Result<Vec<Value>, String> {
        let mut params: Vec<(&str, String)> = vec![];
        if let Some(id) = ticket_id {
            params.push(("ticket_id", id.to_string()));
        }
        let value = self.get_raw("/api/Attachment", &params).await?;
        Ok(parse_halo_list::<Value>(value))
    }

    pub async fn get_attachment(&self, attachment_id: i64) -> Result<Value, String> {
        self.get_raw(&format!("/api/Attachment/{attachment_id}"), &[]).await
    }

    pub async fn delete_attachment(&self, attachment_id: i64) -> Result<(), String> {
        self.delete(&format!("/api/Attachment/{attachment_id}"), &[]).await
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

    /// List ticket areas (e.g. Service Desk, Projects, Internal Processes).
    /// Confirmed against the agent UI's own request.
    pub async fn list_ticket_areas(&self) -> Result<Vec<Value>, String> {
        let value = self
            .get_raw("/api/TicketArea", &[("showall", "true".into()), ("location", "0".into())])
            .await?;
        Ok(parse_halo_list::<Value>(value))
    }

    /// List the steps (and their available transition actions) for a
    /// workflow. Thin wrapper over the existing get_workflow — no new
    /// endpoint, just exposing data we already fetch.
    /// List a workflow's steps. Uses the same lenient raw-JSON path as
    /// get_workflow_details (not the strict typed get_workflow) because
    /// some workflows' step/stage/action objects omit fields the typed
    /// Workflow struct requires, which hard-fails deserialization.
    pub async fn list_workflow_steps(&self, workflow_id: i64) -> Result<Vec<Value>, String> {
        let workflow = self.get_workflow_details(workflow_id).await?;
        Ok(workflow
            .get("steps")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default())
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

    /// Get current user (agent) info. Confirmed against the agent UI:
    /// it calls /api/agent/me on every page load to resolve the logged-in
    /// agent's identity — the same mechanism the app itself uses. This
    /// replaces the previous best-effort approach (decoding JWT claims
    /// from the access token, since /api/AuthInfo only returns server/
    /// tenant metadata, not agent identity). Falls back to that old
    /// approach if /api/agent/me fails for any reason.
    pub async fn get_me(&self) -> Result<Value, String> {
        if let Ok(result) = self.get_no_params("/api/agent/me").await {
            return Ok(result);
        }
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
        let mut records = parse_halo_list::<Value>(value);
        // /api/Agent ignores page_size server-side — truncate client-side.
        records.truncate(page_size.max(1) as usize);
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
        let mut records = parse_halo_list::<Value>(value.clone());
        // record_count is unreliable on this endpoint (observed 0 despite
        // non-empty records) — fall back to the untruncated record count.
        let record_count = value
            .get("record_count")
            .and_then(|v| v.as_i64())
            .filter(|&c| c > 0)
            .unwrap_or(records.len() as i64);
        // /api/AssetGroup ignores page_size server-side — truncate client-side.
        records.truncate(page_size.max(1) as usize);
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

    /// List the priority levels defined on an SLA. Confirmed shape from
    /// the agent UI's rendered Priorities tab (level, description,
    /// response target, resolution target) — the exact JSON field name
    /// wrapping this array in the API response is NOT confirmed (bearer
    /// auth blocks direct API navigation, and DevTools isn't reachable via
    /// browser automation). Tries the likely "priorities" key first,
    /// falling back to the first array field found in the SLA response.
    pub async fn list_priorities(&self, sla_id: i64) -> Result<Vec<Value>, String> {
        let sla = self.get_sla(sla_id).await?;
        if let Some(arr) = sla.get("priorities").and_then(|v| v.as_array()) {
            return Ok(arr.clone());
        }
        if let Some(obj) = sla.as_object() {
            if let Some(arr) = obj.values().find_map(|v| v.as_array()) {
                return Ok(arr.clone());
            }
        }
        Ok(Vec::new())
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
    pub priority: Option<String>,
    pub ticketarea_id: Option<i64>,
    pub parent_id: Option<i64>,
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
