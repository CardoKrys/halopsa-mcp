use serde::{Deserialize, Serialize};

// --- Ticket types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticket {
    pub id: i64,
    pub summary: String,
    #[serde(default)]
    pub details: String,
    #[serde(default)]
    pub client_id: Option<i64>,
    #[serde(default)]
    pub client_name: Option<String>,
    #[serde(default)]
    pub site_id: Option<i64>,
    #[serde(default)]
    pub site_name: Option<String>,
    #[serde(default)]
    pub user_id: Option<i64>,
    #[serde(default)]
    pub user_name: Option<String>,
    #[serde(default)]
    pub agent_id: Option<i64>,
    #[serde(default)]
    pub agent_name: Option<String>,
    #[serde(default)]
    pub team: Option<String>,
    #[serde(default)]
    pub team_id: Option<i64>,
    #[serde(default)]
    pub status_id: Option<i64>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub priority_id: Option<i64>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub tickettype_id: Option<i64>,
    #[serde(default)]
    pub tickettype_name: Option<String>,
    #[serde(default)]
    pub category_1: Option<String>,
    #[serde(default)]
    pub category_2: Option<String>,
    #[serde(default)]
    pub category_3: Option<String>,
    #[serde(default)]
    pub workflow_step: Option<i64>,
    #[serde(default)]
    pub dateoccurred: Option<String>,
    #[serde(default)]
    pub dateclosed: Option<String>,
    #[serde(default)]
    pub sla_name: Option<String>,
    #[serde(default)]
    pub customfields: Option<Vec<CustomField>>,
    /// Raw JSON from HaloPSA for fields we don't explicitly model
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomField {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub value: Option<serde_json::Value>,
    #[serde(default)]
    pub display: Option<String>,
}

// --- Action types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub id: i64,
    pub ticket_id: i64,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub note_html: Option<String>,
    #[serde(default)]
    pub who: Option<String>,
    #[serde(default)]
    pub who_agentid: Option<i64>,
    #[serde(default)]
    pub outcome: Option<String>,
    #[serde(default)]
    pub outcome_id: Option<i64>,
    #[serde(default)]
    pub hiddenfromuser: bool,
    #[serde(default)]
    pub isimportant: bool,
    #[serde(default)]
    pub actiondate: Option<String>,
    #[serde(default)]
    pub workflow_subdetail_id: Option<i64>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// --- Workflow types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub steps: Vec<WorkflowStep>,
    #[serde(default)]
    pub stages: Vec<WorkflowStage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub stage_number: Option<i64>,
    #[serde(default)]
    pub actions: Vec<WorkflowAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowAction {
    pub id: i64,
    #[serde(default)]
    pub action_name: String,
    #[serde(default)]
    pub start_step: Option<i64>,
    #[serde(default)]
    pub end_step: Option<i64>,
    #[serde(default)]
    pub action_outcome: Option<String>,
    #[serde(default)]
    pub action_colour: Option<String>,
    /// 0 = not available to end users, >= 1 = available
    #[serde(default)]
    pub enduseraction: i64,
    /// 0 = not available to agents, >= 1 = available
    #[serde(default)]
    pub agentaction: Option<i64>,
    #[serde(default)]
    pub action_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStage {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub sequence: i64,
    #[serde(default)]
    pub stage_number: Option<i64>,
}

// --- Embedding types ---

#[derive(Debug, Clone)]
pub struct TicketMeta {
    pub ticket_id: i64,
    pub summary: String,
    pub content_hash: String,
    pub client_name: Option<String>,
    pub agent_name: Option<String>,
    pub status: Option<String>,
    pub team: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ChunkInsert {
    pub content: String,
    pub content_hash: String,
    pub embedding: Vec<f32>,
    pub heading_path: String,
}

#[derive(Debug, Clone)]
pub struct ChunkDetail {
    pub chunk_id: i64,
    pub ticket_id: i64,
    pub content: String,
    pub heading_path: String,
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub chunk_id: i64,
    pub ticket_id: i64,
    pub score: f32,
}

#[derive(Debug, Clone, Default)]
pub struct EmbedStats {
    pub total_indexed_tickets: i64,
    pub total_chunks: i64,
}

#[derive(Debug, Clone)]
pub struct EmbedJob {
    pub id: i64,
    pub scope: String,
    pub status: String,
    pub done_tickets: i64,
    pub total_tickets: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub error: Option<String>,
    pub worker_id: Option<String>,
}

// --- API list response ---

/// HaloPSA returns lists as either a bare array, or an object like
/// `{ record_count, tickets: [...] }` / `{ record_count, actions: [...] }`.
/// The array's key varies per endpoint (it's the resource name, not a fixed
/// "records" key), so find the first array-valued field rather than assuming
/// one specific key name.
pub fn parse_halo_list<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> Vec<T> {
    if let Some(arr) = value.as_array() {
        return arr
            .iter()
            .filter_map(|v| serde_json::from_value(v.clone()).ok())
            .collect();
    }
    if let Some(obj) = value.as_object() {
        if let Some(arr) = obj.values().find_map(|v| v.as_array()) {
            return arr
                .iter()
                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                .collect();
        }
    }
    Vec::new()
}
