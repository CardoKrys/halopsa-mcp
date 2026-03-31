use serde_json::{json, Value};

use hmcp_common::halopsa::{HaloPSAClient, TicketFilter};

use crate::semantic::SemanticState;

/// Core tool definitions (always available).
pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "list_tickets",
            "description": "List tickets with optional filters. Returns paginated results. Use filters to narrow by agent, team/queue, client, status, or keyword search.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 },
                    "search": { "type": "string", "description": "Keyword search across ticket summary and details" },
                    "agent_id": { "type": "integer", "description": "Filter by assigned agent ID" },
                    "team_id": { "type": "integer", "description": "Filter by team/queue ID" },
                    "client_id": { "type": "integer", "description": "Filter by client ID" },
                    "status_id": { "type": "integer", "description": "Filter by status ID" },
                    "tickettype_id": { "type": "integer", "description": "Filter by ticket type ID" },
                    "open_only": { "type": "boolean", "description": "Only return open tickets (default false)", "default": false }
                }
            }
        }),
        json!({
            "name": "get_ticket",
            "description": "Get full details of a single ticket including custom fields, workflow state, and related information.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ticket_id": { "type": "integer", "description": "The ticket ID" }
                },
                "required": ["ticket_id"]
            }
        }),
        json!({
            "name": "create_ticket",
            "description": "Create a new ticket. At minimum requires a summary and client_id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "summary": { "type": "string", "description": "Ticket subject/summary" },
                    "details": { "type": "string", "description": "Ticket description/details (can include HTML)" },
                    "client_id": { "type": "integer", "description": "Client ID the ticket belongs to" },
                    "tickettype_id": { "type": "integer", "description": "Ticket type ID" },
                    "site_id": { "type": "integer", "description": "Site ID" },
                    "user_id": { "type": "integer", "description": "End user/contact ID" },
                    "agent_id": { "type": "integer", "description": "Assign to this agent" },
                    "team_id": { "type": "integer", "description": "Assign to this team/queue" },
                    "priority_id": { "type": "integer", "description": "Priority ID" },
                    "category_1": { "type": "string", "description": "Primary category" },
                    "category_2": { "type": "string", "description": "Secondary category" },
                    "category_3": { "type": "string", "description": "Tertiary category" }
                },
                "required": ["summary", "client_id"]
            }
        }),
        json!({
            "name": "update_ticket",
            "description": "Update fields on an existing ticket. Only provided fields are changed.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ticket_id": { "type": "integer", "description": "The ticket ID to update" },
                    "summary": { "type": "string", "description": "New ticket summary" },
                    "details": { "type": "string", "description": "New ticket details" },
                    "agent_id": { "type": "integer", "description": "Reassign to this agent" },
                    "team_id": { "type": "integer", "description": "Move to this team/queue" },
                    "status_id": { "type": "integer", "description": "Change status" },
                    "priority_id": { "type": "integer", "description": "Change priority" },
                    "category_1": { "type": "string", "description": "Change primary category" },
                    "category_2": { "type": "string", "description": "Change secondary category" },
                    "category_3": { "type": "string", "description": "Change tertiary category" }
                },
                "required": ["ticket_id"]
            }
        }),
        json!({
            "name": "search_tickets",
            "description": "Search tickets by keyword. Returns matching tickets ordered by relevance.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "page_size": { "type": "integer", "description": "Max results (default 20)", "default": 20 }
                },
                "required": ["query"]
            }
        }),
        json!({
            "name": "list_actions",
            "description": "List all actions (notes, replies, workflow transitions) on a ticket.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ticket_id": { "type": "integer", "description": "The ticket ID" },
                    "include_private": { "type": "boolean", "description": "Include private/internal notes (default false)", "default": false }
                },
                "required": ["ticket_id"]
            }
        }),
        json!({
            "name": "create_action",
            "description": "Add a note or reply to a ticket. Can optionally include a workflow transition.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ticket_id": { "type": "integer", "description": "The ticket ID" },
                    "note": { "type": "string", "description": "The note/reply content" },
                    "outcome": { "type": "string", "description": "Action outcome (default 'note')", "default": "note" },
                    "workflow_action_id": { "type": "integer", "description": "Workflow action ID to execute (from get_available_actions)" },
                    "hidden_from_user": { "type": "boolean", "description": "Hide this note from end users (default false)", "default": false }
                },
                "required": ["ticket_id", "note"]
            }
        }),
        json!({
            "name": "get_available_actions",
            "description": "Get the workflow actions available on a ticket's current step. Returns actions the authenticated user can execute.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ticket_id": { "type": "integer", "description": "The ticket ID" }
                },
                "required": ["ticket_id"]
            }
        }),
        json!({
            "name": "execute_workflow_action",
            "description": "Execute a specific workflow action on a ticket, transitioning it to the next step. Use get_available_actions first to see valid options.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ticket_id": { "type": "integer", "description": "The ticket ID" },
                    "action_id": { "type": "integer", "description": "The workflow action ID (from get_available_actions)" },
                    "note": { "type": "string", "description": "Optional note to include with the action" }
                },
                "required": ["ticket_id", "action_id"]
            }
        }),
        json!({
            "name": "list_statuses",
            "description": "List all available ticket statuses with their IDs.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "list_teams",
            "description": "List all teams/queues the user has access to.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "list_ticket_types",
            "description": "List all available ticket types.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "get_client",
            "description": "Get details about a specific client by ID.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "client_id": { "type": "integer", "description": "The client ID" }
                },
                "required": ["client_id"]
            }
        }),
        json!({
            "name": "get_me",
            "description": "Get information about the authenticated user (agent).",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "get_ticket_assets",
            "description": "List assets associated with a ticket.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ticket_id": { "type": "integer", "description": "The ticket ID" }
                },
                "required": ["ticket_id"]
            }
        }),
    ]
}

/// Semantic search tools (only when semantic search is enabled).
pub fn semantic_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "semantic_search",
            "description": "Search tickets using semantic/meaning-based search. Finds conceptually related tickets even when keywords don't match exactly. Results are filtered to tickets the user has access to.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Natural language search query" },
                    "limit": { "type": "integer", "description": "Maximum results (default 10)", "default": 10 }
                },
                "required": ["query"]
            }
        }),
        json!({
            "name": "embedding_status",
            "description": "Check the status of the ticket embedding/indexing system.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "reembed",
            "description": "Trigger re-indexing of all tickets for semantic search.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "scope": { "type": "string", "description": "Scope: 'all' for full reindex, or 'ticket:{id}' for a specific ticket", "default": "all" }
                }
            }
        }),
    ]
}

/// Execute a tool by name. Returns Ok(text) on success, Err(text) on failure.
pub async fn execute_tool(
    name: &str,
    args: &Value,
    client: &HaloPSAClient,
    semantic: Option<&SemanticState>,
) -> Result<String, String> {
    match name {
        "list_tickets" => exec_list_tickets(args, client).await,
        "get_ticket" => exec_get_ticket(args, client).await,
        "create_ticket" => exec_create_ticket(args, client).await,
        "update_ticket" => exec_update_ticket(args, client).await,
        "search_tickets" => exec_search_tickets(args, client).await,
        "list_actions" => exec_list_actions(args, client).await,
        "create_action" => exec_create_action(args, client).await,
        "get_available_actions" => exec_get_available_actions(args, client).await,
        "execute_workflow_action" => exec_execute_workflow_action(args, client).await,
        "list_statuses" => exec_list_statuses(client).await,
        "list_teams" => exec_list_teams(client).await,
        "list_ticket_types" => exec_list_ticket_types(client).await,
        "get_client" => exec_get_client(args, client).await,
        "get_me" => exec_get_me(client).await,
        "get_ticket_assets" => exec_get_ticket_assets(args, client).await,
        "semantic_search" => {
            let sem = semantic.ok_or("Semantic search not enabled")?;
            exec_semantic_search(args, client, sem).await
        }
        "embedding_status" => {
            let sem = semantic.ok_or("Semantic search not enabled")?;
            exec_embedding_status(sem).await
        }
        "reembed" => {
            let sem = semantic.ok_or("Semantic search not enabled")?;
            exec_reembed(args, sem).await
        }
        _ => Err(format!("Unknown tool: {name}")),
    }
}

// --- Tool implementations ---

async fn exec_list_tickets(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);

    let filter = TicketFilter {
        search: args.get("search").and_then(|v| v.as_str()).map(String::from),
        agent_id: args.get("agent_id").and_then(|v| v.as_i64()),
        team_id: args.get("team_id").and_then(|v| v.as_i64()),
        client_id: args.get("client_id").and_then(|v| v.as_i64()),
        status_id: args.get("status_id").and_then(|v| v.as_i64()),
        tickettype_id: args.get("tickettype_id").and_then(|v| v.as_i64()),
        open_only: args.get("open_only").and_then(|v| v.as_bool()).unwrap_or(false),
    };

    let (tickets, total) = client.list_tickets(page, page_size, &filter).await?;

    let summary: Vec<Value> = tickets
        .iter()
        .map(|t| {
            json!({
                "id": t.get("id"),
                "summary": t.get("summary"),
                "client_name": t.get("client_name"),
                "agent_name": t.get("agent_name"),
                "team": t.get("team"),
                "status": t.get("status"),
                "priority": t.get("priority"),
                "dateoccurred": t.get("dateoccurred"),
            })
        })
        .collect();

    Ok(serde_json::to_string_pretty(&json!({
        "tickets": summary,
        "total_count": total,
        "page": page,
        "page_size": page_size,
    }))
    .unwrap())
}

async fn exec_get_ticket(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let ticket_id = args
        .get("ticket_id")
        .and_then(|v| v.as_i64())
        .ok_or("ticket_id is required")?;

    let ticket = client.get_ticket(ticket_id).await?;

    // Also fetch available actions
    let actions = client.get_available_actions(ticket_id).await.unwrap_or_default();
    let action_names: Vec<String> = actions.iter().map(|a| {
        format!("{} (id: {})", a.action_name, a.id)
    }).collect();

    let mut result = ticket;
    if let Some(obj) = result.as_object_mut() {
        obj.insert("_available_workflow_actions".into(), json!(action_names));
    }

    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_create_ticket(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let summary = args
        .get("summary")
        .and_then(|v| v.as_str())
        .ok_or("summary is required")?;
    let client_id = args
        .get("client_id")
        .and_then(|v| v.as_i64())
        .ok_or("client_id is required")?;

    let mut ticket = json!({
        "summary": summary,
        "client_id": client_id,
    });

    // Copy optional fields
    for field in &[
        "details",
        "tickettype_id",
        "site_id",
        "user_id",
        "agent_id",
        "team_id",
        "priority_id",
        "category_1",
        "category_2",
        "category_3",
    ] {
        if let Some(val) = args.get(*field) {
            ticket[*field] = val.clone();
        }
    }

    let result = client.create_ticket(ticket).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_update_ticket(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let ticket_id = args
        .get("ticket_id")
        .and_then(|v| v.as_i64())
        .ok_or("ticket_id is required")?;

    let mut fields = json!({});
    for field in &[
        "summary",
        "details",
        "agent_id",
        "team_id",
        "status_id",
        "priority_id",
        "category_1",
        "category_2",
        "category_3",
    ] {
        if let Some(val) = args.get(*field) {
            fields[*field] = val.clone();
        }
    }

    let result = client.update_ticket(ticket_id, fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_search_tickets(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or("query is required")?;
    let page_size = args
        .get("page_size")
        .and_then(|v| v.as_i64())
        .unwrap_or(20);

    let (tickets, total) = client.search_tickets(query, page_size).await?;

    let summary: Vec<Value> = tickets
        .iter()
        .map(|t| {
            json!({
                "id": t.get("id"),
                "summary": t.get("summary"),
                "client_name": t.get("client_name"),
                "status": t.get("status"),
                "agent_name": t.get("agent_name"),
            })
        })
        .collect();

    Ok(serde_json::to_string_pretty(&json!({
        "results": summary,
        "total_count": total,
    }))
    .unwrap())
}

async fn exec_list_actions(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let ticket_id = args
        .get("ticket_id")
        .and_then(|v| v.as_i64())
        .ok_or("ticket_id is required")?;
    let include_private = args
        .get("include_private")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let actions = client.list_actions(ticket_id, include_private).await?;

    let summary: Vec<Value> = actions
        .iter()
        .map(|a| {
            json!({
                "id": a.get("id"),
                "who": a.get("who"),
                "outcome": a.get("outcome"),
                "note": a.get("note"),
                "actiondate": a.get("actiondate"),
                "hiddenfromuser": a.get("hiddenfromuser"),
                "isimportant": a.get("isimportant"),
            })
        })
        .collect();

    Ok(serde_json::to_string_pretty(&json!({
        "actions": summary,
        "count": actions.len(),
    }))
    .unwrap())
}

async fn exec_create_action(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let ticket_id = args
        .get("ticket_id")
        .and_then(|v| v.as_i64())
        .ok_or("ticket_id is required")?;
    let note = args
        .get("note")
        .and_then(|v| v.as_str())
        .ok_or("note is required")?;
    let outcome = args
        .get("outcome")
        .and_then(|v| v.as_str())
        .unwrap_or("note");
    let workflow_action_id = args.get("workflow_action_id").and_then(|v| v.as_i64());
    let hidden = args
        .get("hidden_from_user")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let result = client
        .create_action(ticket_id, note, outcome, workflow_action_id, hidden)
        .await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_get_available_actions(
    args: &Value,
    client: &HaloPSAClient,
) -> Result<String, String> {
    let ticket_id = args
        .get("ticket_id")
        .and_then(|v| v.as_i64())
        .ok_or("ticket_id is required")?;

    let actions = client.get_available_actions(ticket_id).await?;

    let summary: Vec<Value> = actions
        .iter()
        .map(|a| {
            json!({
                "id": a.id,
                "name": a.action_name,
                "outcome": a.action_outcome,
                "end_step": a.end_step,
                "colour": a.action_colour,
            })
        })
        .collect();

    Ok(serde_json::to_string_pretty(&json!({
        "available_actions": summary,
        "count": actions.len(),
        "hint": "Use execute_workflow_action with the action id to transition the ticket"
    }))
    .unwrap())
}

async fn exec_execute_workflow_action(
    args: &Value,
    client: &HaloPSAClient,
) -> Result<String, String> {
    let ticket_id = args
        .get("ticket_id")
        .and_then(|v| v.as_i64())
        .ok_or("ticket_id is required")?;
    let action_id = args
        .get("action_id")
        .and_then(|v| v.as_i64())
        .ok_or("action_id is required")?;
    let note = args.get("note").and_then(|v| v.as_str()).unwrap_or("");

    let result = client
        .create_action(ticket_id, note, "note", Some(action_id), false)
        .await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_statuses(client: &HaloPSAClient) -> Result<String, String> {
    let statuses = client.list_statuses().await?;
    let summary: Vec<Value> = statuses
        .iter()
        .map(|s| {
            json!({
                "id": s.get("id"),
                "name": s.get("name"),
            })
        })
        .collect();
    Ok(serde_json::to_string_pretty(&json!({ "statuses": summary })).unwrap())
}

async fn exec_list_teams(client: &HaloPSAClient) -> Result<String, String> {
    let teams = client.list_teams().await?;
    let summary: Vec<Value> = teams
        .iter()
        .map(|t| {
            json!({
                "id": t.get("id"),
                "name": t.get("name"),
            })
        })
        .collect();
    Ok(serde_json::to_string_pretty(&json!({ "teams": summary })).unwrap())
}

async fn exec_list_ticket_types(client: &HaloPSAClient) -> Result<String, String> {
    let types = client.list_ticket_types().await?;
    let summary: Vec<Value> = types
        .iter()
        .map(|t| {
            json!({
                "id": t.get("id"),
                "name": t.get("name"),
            })
        })
        .collect();
    Ok(serde_json::to_string_pretty(&json!({ "ticket_types": summary })).unwrap())
}

async fn exec_get_client(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let client_id = args
        .get("client_id")
        .and_then(|v| v.as_i64())
        .ok_or("client_id is required")?;

    let result = client.get_client(client_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_get_me(client: &HaloPSAClient) -> Result<String, String> {
    let result = client.get_me().await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_get_ticket_assets(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let ticket_id = args
        .get("ticket_id")
        .and_then(|v| v.as_i64())
        .ok_or("ticket_id is required")?;

    let assets = client.get_ticket_assets(ticket_id).await?;
    let summary: Vec<Value> = assets
        .iter()
        .map(|a| {
            json!({
                "id": a.get("id"),
                "inventory_number": a.get("inventory_number"),
                "asset_tag": a.get("asset_tag"),
                "devicename": a.get("devicename"),
                "assettype_name": a.get("assettype_name"),
                "client_name": a.get("client_name"),
            })
        })
        .collect();
    Ok(serde_json::to_string_pretty(&json!({ "assets": summary })).unwrap())
}

// --- Semantic search tools ---

async fn exec_semantic_search(
    args: &Value,
    client: &HaloPSAClient,
    semantic: &SemanticState,
) -> Result<String, String> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or("query is required")?;
    let limit = args
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(10) as usize;

    let results = semantic.search(query, limit, client).await?;
    Ok(serde_json::to_string_pretty(&results).unwrap())
}

async fn exec_embedding_status(semantic: &SemanticState) -> Result<String, String> {
    let status = semantic.embedding_status().await?;
    Ok(serde_json::to_string_pretty(&status).unwrap())
}

async fn exec_reembed(args: &Value, semantic: &SemanticState) -> Result<String, String> {
    let scope = args
        .get("scope")
        .and_then(|v| v.as_str())
        .unwrap_or("all");

    let (job_id, is_new) = semantic.queue_embed_job(scope).await?;
    Ok(serde_json::to_string_pretty(&json!({
        "job_id": job_id,
        "is_new": is_new,
        "scope": scope,
        "message": if is_new { "Embed job queued" } else { "Existing job found" }
    }))
    .unwrap())
}
