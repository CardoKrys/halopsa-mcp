use serde_json::{json, Value};

use hmcp_common::halopsa::HaloPSAClient;

use crate::semantic::SemanticState;
use crate::tools;

const PROTOCOL_VERSION: &str = "2025-03-26";

/// Handle a JSON-RPC request and return a response (or None for notifications).
pub async fn handle_request(
    request: &Value,
    client: &HaloPSAClient,
    semantic: Option<&SemanticState>,
) -> Option<Value> {
    let method = request.get("method")?.as_str()?;
    let id = request.get("id").cloned();
    let params = request.get("params").cloned().unwrap_or(json!({}));

    match method {
        "initialize" => {
            let mut tool_list = tools::tool_definitions();
            if semantic.is_some() {
                tool_list.extend(tools::semantic_tool_definitions());
            }

            Some(jsonrpc_response(
                id,
                json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "halopsa-mcp",
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "instructions": build_instructions(semantic.is_some())
                }),
            ))
        }
        "tools/list" => {
            let mut all_tools = tools::tool_definitions();
            if semantic.is_some() {
                all_tools.extend(tools::semantic_tool_definitions());
            }
            Some(jsonrpc_response(id, json!({ "tools": all_tools })))
        }
        "tools/call" => {
            let name = params.get("name")?.as_str()?;
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or(json!({}));

            let result = tools::execute_tool(name, &args, client, semantic).await;

            let (content, is_error) = match result {
                Ok(text) => (text, false),
                Err(err) => (err, true),
            };

            Some(jsonrpc_response(
                id,
                json!({
                    "content": [{
                        "type": "text",
                        "text": content
                    }],
                    "isError": is_error
                }),
            ))
        }
        "notifications/initialized" => None,
        _ => Some(jsonrpc_error(id, -32601, "Method not found")),
    }
}

fn jsonrpc_response(id: Option<Value>, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn jsonrpc_error(id: Option<Value>, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

fn build_instructions(semantic_enabled: bool) -> String {
    let mut instructions = String::from(
        "HaloPSA MCP Server — interact with HaloPSA tickets, actions, and workflows.\n\n\
         IMPORTANT: All operations respect the authenticated user's HaloPSA permissions. \
         You can only see and modify tickets, queues, and clients that the user has access to.\n\n\
         ## Ticket Management\n\
         - Use `list_tickets` to find tickets with filters (by queue, agent, client, status)\n\
         - Use `get_ticket` to get full details including custom fields and workflow state\n\
         - Use `create_ticket` to create new tickets\n\
         - Use `update_ticket` to modify ticket fields\n\
         - Use `search_tickets` for keyword search\n\n\
         ## Actions & Workflow\n\
         - Use `list_actions` to see the action history on a ticket\n\
         - Use `get_available_actions` to see what workflow transitions are possible\n\
         - Use `create_action` to add notes or replies\n\
         - Use `execute_workflow_action` to transition a ticket through its workflow\n\n\
         ## Supporting Data\n\
         - Use `list_statuses`, `list_teams`, `list_ticket_types` for reference data\n\
         - Use `get_client` for client details\n\
         - Use `get_me` for the authenticated user's info\n"
    );

    if semantic_enabled {
        instructions.push_str(
            "\n## Semantic Search\n\
             - Use `semantic_search` to find tickets by meaning, not just keywords\n\
             - Use `embedding_status` to check the indexing status\n\
             - Use `reembed` to trigger re-indexing of all tickets\n"
        );
    }

    instructions
}
