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
            "name": "add_action",
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
            "name": "send_email_reply",
            "description": "Send a visible email reply to the ticket contact. The reply is sent from your HaloPSA account and is visible to the end user.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ticket_id": { "type": "integer", "description": "The ticket ID" },
                    "message": { "type": "string", "description": "The reply message content" }
                },
                "required": ["ticket_id", "message"]
            }
        }),
        json!({
            "name": "add_internal_note",
            "description": "Add a private internal note to a ticket. Visible to agents only — not sent to the end user.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ticket_id": { "type": "integer", "description": "The ticket ID" },
                    "note": { "type": "string", "description": "The internal note content" }
                },
                "required": ["ticket_id", "note"]
            }
        }),
        json!({
            "name": "change_ticket_status_with_note",
            "description": "Change a ticket's status and add a note in one operation.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ticket_id": { "type": "integer", "description": "The ticket ID" },
                    "status_id": { "type": "integer", "description": "The new status ID (use list_statuses to find valid IDs)" },
                    "note": { "type": "string", "description": "Note to add alongside the status change" },
                    "hidden_from_user": { "type": "boolean", "description": "Hide the note from the end user (default false)", "default": false }
                },
                "required": ["ticket_id", "status_id", "note"]
            }
        }),
        json!({
            "name": "list_ticket_actions",
            "description": "List all actions (notes, replies, workflow transitions) on a ticket. Alias for list_actions.",
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
            "name": "log_time",
            "description": "Log time spent on a ticket. Time is recorded as an action/note entry.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ticket_id": { "type": "integer", "description": "The ticket ID" },
                    "time_minutes": { "type": "number", "description": "Time spent in minutes (e.g. 30 for half an hour, 90 for 1.5 hours)" },
                    "note": { "type": "string", "description": "Description of work done (optional)", "default": "" },
                    "outcome": { "type": "string", "description": "Action outcome (default 'note')", "default": "note" },
                    "hidden_from_user": { "type": "boolean", "description": "Hide from end user (default true for time entries)", "default": true }
                },
                "required": ["ticket_id", "time_minutes"]
            }
        }),
        json!({
            "name": "update_action",
            "description": "Edit the text of an existing action (note or reply) on a ticket.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ticket_id": { "type": "integer", "description": "The ticket ID the action belongs to" },
                    "action_id": { "type": "integer", "description": "The action ID to update" },
                    "note": { "type": "string", "description": "The new note content" },
                    "hidden_from_user": { "type": "boolean", "description": "Change visibility — true hides from end user" }
                },
                "required": ["ticket_id", "action_id", "note"]
            }
        }),
        json!({
            "name": "delete_action",
            "description": "Delete an action (note or reply) from a ticket by its ID. Requires appropriate HaloPSA permissions.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ticket_id": { "type": "integer", "description": "The ticket ID the action belongs to" },
                    "action_id": { "type": "integer", "description": "The action ID to delete" }
                },
                "required": ["ticket_id", "action_id"]
            }
        }),
        json!({
            "name": "get_action",
            "description": "Get full details of a single action (note, reply, or transition) by its ID.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ticket_id": { "type": "integer", "description": "The ticket ID the action belongs to" },
                    "action_id": { "type": "integer", "description": "The action ID" }
                },
                "required": ["ticket_id", "action_id"]
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
            "name": "list_clients",
            "description": "List clients with optional keyword search. Returns paginated results.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 },
                    "search": { "type": "string", "description": "Keyword search across client name" }
                }
            }
        }),
        json!({
            "name": "search_clients",
            "description": "Search clients by keyword. Returns matching clients ordered by relevance.",
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
            "name": "list_users",
            "description": "List users (end-user contacts) with optional client filter and keyword search. Returns paginated results.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 },
                    "client_id": { "type": "integer", "description": "Filter by client ID" },
                    "search": { "type": "string", "description": "Keyword search across user name/email" }
                }
            }
        }),
        json!({
            "name": "get_user",
            "description": "Get full details of a single user (end-user contact) by ID.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "user_id": { "type": "integer", "description": "The user ID" }
                },
                "required": ["user_id"]
            }
        }),
        json!({
            "name": "search_users",
            "description": "Search users (end-user contacts) by keyword. Returns matching users.",
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
            "name": "list_reports",
            "description": "List saved report definitions within a category (use list_report_categories to find category IDs, or pass 0 for All Reports), or search by name across every category. When search is provided, it takes priority and reportgroup_id is ignored — search spans all categories. Returns paginated results.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 },
                    "reportgroup_id": { "type": "integer", "description": "Category ID from list_report_categories (default 0 = All Reports). Ignored if search is set.", "default": 0 },
                    "search": { "type": "string", "description": "Keyword search on report name, across all categories" }
                }
            }
        }),
        json!({
            "name": "list_report_categories",
            "description": "List report categories/groups (e.g. 'My Reports', 'KPI Reports'). Use the returned id as reportgroup_id in list_reports.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "get_lookup_values",
            "description": "Get values for a HaloPSA lookup table by ID (e.g. lookupid 41 is report categories). Use when you need a specific lookup table's contents and know its ID.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "lookupid": { "type": "integer", "description": "The lookup table ID" },
                    "istree": { "type": "boolean", "description": "Whether to return the tree-structured form (default true)", "default": true }
                },
                "required": ["lookupid"]
            }
        }),
        json!({
            "name": "get_agent",
            "description": "Get details about a specific agent (staff member) by ID.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "agent_id": { "type": "integer", "description": "The agent ID" }
                },
                "required": ["agent_id"]
            }
        }),
        json!({
            "name": "list_workflow_steps",
            "description": "List the steps and available transition actions for a workflow. Use get_ticket_type or a ticket's workflow_id to find the workflow ID.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workflow_id": { "type": "integer", "description": "The workflow ID" }
                },
                "required": ["workflow_id"]
            }
        }),
        json!({
            "name": "run_report",
            "description": "Run a saved HaloPSA report by ID and return its result rows. Use list_reports to find report IDs. The response includes filterable_columns — only filter on fields listed there. To narrow results, pass filters overriding the report's saved filter set.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "report_id": { "type": "integer", "description": "The report ID (from list_reports)" },
                    "filters": {
                        "type": "array",
                        "description": "Override the report's filters. Each item filters one column. Only stringruletype 0 (Includes) and 1 (Does not include) are confirmed — avoid other values until confirmed.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "fieldname": { "type": "string", "description": "Column name to filter on — must be one of the report's filterable_columns" },
                                "stringruletype": { "type": "integer", "description": "0 = Includes, 1 = Does not include" },
                                "stringrulevalues": { "type": "array", "items": { "type": "string" }, "description": "One or more values to match against this column" }
                            },
                            "required": ["fieldname", "stringruletype", "stringrulevalues"]
                        }
                    },
                    "parameters": {
                        "type": "object",
                        "description": "Optional report-specific query parameters as key-value pairs (varies per report — e.g. some reports take a 'clientname' filter). Unconfirmed whether these actually affect results — prefer filters for narrowing by column.",
                        "additionalProperties": { "type": "string" }
                    }
                },
                "required": ["report_id"]
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
        json!({
            "name": "get_asset",
            "description": "Get full details of a single asset by ID.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "asset_id": { "type": "integer", "description": "The asset ID" }
                },
                "required": ["asset_id"]
            }
        }),
        json!({
            "name": "list_assets",
            "description": "List assets, optionally filtered by asset type. Omit assettype_id for all types. Returns paginated results.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 },
                    "assettype_id": { "type": "integer", "description": "Filter by asset type ID. Omit for all types." },
                    "search": { "type": "string", "description": "Keyword search across assets" }
                }
            }
        }),
        json!({
            "name": "search_assets",
            "description": "Search assets by keyword across all types. Returns matching assets.",
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
            "name": "list_sites",
            "description": "List sites. Returns paginated results.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "get_site",
            "description": "Get full details of a single site by ID.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "site_id": { "type": "integer", "description": "The site ID" }
                },
                "required": ["site_id"]
            }
        }),
        json!({
            "name": "create_site",
            "description": "Create a new site for a client.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "client_id": { "type": "integer", "description": "The client this site belongs to" },
                    "name": { "type": "string", "description": "Site name" },
                    "address_line1": { "type": "string", "description": "Address line 1" },
                    "address_line2": { "type": "string", "description": "Address line 2" },
                    "address_line3": { "type": "string", "description": "Address line 3" },
                    "address_line4": { "type": "string", "description": "Address line 4" },
                    "address_postcode": { "type": "string", "description": "Postcode" },
                    "sla_id": { "type": "integer", "description": "SLA ID to apply to this site" },
                    "timezone": { "type": "string", "description": "Timezone name" },
                    "inactive": { "type": "boolean", "description": "Mark as inactive (default false)" }
                },
                "required": ["client_id", "name"]
            }
        }),
        json!({
            "name": "update_site",
            "description": "Update fields on an existing site. Only provided fields are changed.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "site_id": { "type": "integer", "description": "The site ID to update" },
                    "name": { "type": "string", "description": "New site name" },
                    "address_line1": { "type": "string", "description": "Address line 1" },
                    "address_line2": { "type": "string", "description": "Address line 2" },
                    "address_line3": { "type": "string", "description": "Address line 3" },
                    "address_line4": { "type": "string", "description": "Address line 4" },
                    "address_postcode": { "type": "string", "description": "Postcode" },
                    "sla_id": { "type": "integer", "description": "SLA ID to apply to this site" },
                    "timezone": { "type": "string", "description": "Timezone name" },
                    "inactive": { "type": "boolean", "description": "Mark as inactive/active" }
                },
                "required": ["site_id"]
            }
        }),
        json!({
            "name": "list_agents",
            "description": "List agents (staff members). Returns paginated results.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "list_asset_groups",
            "description": "List asset groups. Returns paginated results.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "get_asset_group",
            "description": "Get full details of a single asset group by ID.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "asset_group_id": { "type": "integer", "description": "The asset group ID" }
                },
                "required": ["asset_group_id"]
            }
        }),
        json!({
            "name": "list_slas",
            "description": "List all SLAs (service level agreements).",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "get_sla",
            "description": "Get full details of a single SLA by ID, including its nested priority levels. Tickets are associated with an SLA, and priorities are defined per-SLA rather than globally.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "sla_id": { "type": "integer", "description": "The SLA ID" }
                },
                "required": ["sla_id"]
            }
        }),
        json!({
            "name": "list_outcomes",
            "description": "List all action outcome definitions (e.g. the options available when adding an action to a ticket).",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "list_categories",
            "description": "List categories of a given type. type_id 1 = ticket categories, type_id 2 = resolution categories. There may be other category types on this instance — use list_categories with different type_id values to discover them if needed.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "type_id": { "type": "integer", "description": "Category type ID (1 = ticket categories, 2 = resolution categories)" }
                },
                "required": ["type_id"]
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
        "list_actions" | "list_ticket_actions" => exec_list_actions(args, client).await,
        "add_action" => exec_add_action(args, client).await,
        "send_email_reply" => exec_send_email_reply(args, client).await,
        "add_internal_note" => exec_add_internal_note(args, client).await,
        "change_ticket_status_with_note" => exec_change_ticket_status_with_note(args, client).await,
        "get_action" => exec_get_action(args, client).await,
        "log_time" => exec_log_time(args, client).await,
        "update_action" => exec_update_action(args, client).await,
        "delete_action" => exec_delete_action(args, client).await,
        "get_available_actions" => exec_get_available_actions(args, client).await,
        "execute_workflow_action" => exec_execute_workflow_action(args, client).await,
        "list_statuses" => exec_list_statuses(client).await,
        "list_teams" => exec_list_teams(client).await,
        "list_ticket_types" => exec_list_ticket_types(client).await,
        "get_client" => exec_get_client(args, client).await,
        "list_clients" => exec_list_clients(args, client).await,
        "search_clients" => exec_search_clients(args, client).await,
        "list_users" => exec_list_users(args, client).await,
        "get_user" => exec_get_user(args, client).await,
        "search_users" => exec_search_users(args, client).await,
        "list_reports" => exec_list_reports(args, client).await,
        "list_report_categories" => exec_list_report_categories(client).await,
        "get_lookup_values" => exec_get_lookup_values(args, client).await,
        "get_agent" => exec_get_agent(args, client).await,
        "list_workflow_steps" => exec_list_workflow_steps(args, client).await,
        "get_asset" => exec_get_asset(args, client).await,
        "list_assets" => exec_list_assets(args, client).await,
        "search_assets" => exec_search_assets(args, client).await,
        "list_sites" => exec_list_sites(args, client).await,
        "get_site" => exec_get_site(args, client).await,
        "create_site" => exec_create_site(args, client).await,
        "update_site" => exec_update_site(args, client).await,
        "list_agents" => exec_list_agents(args, client).await,
        "list_asset_groups" => exec_list_asset_groups(args, client).await,
        "get_asset_group" => exec_get_asset_group(args, client).await,
        "list_slas" => exec_list_slas(client).await,
        "get_sla" => exec_get_sla(args, client).await,
        "list_outcomes" => exec_list_outcomes(client).await,
        "list_categories" => exec_list_categories(args, client).await,
        "run_report" => exec_run_report(args, client).await,
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

async fn exec_add_action(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
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
        .create_action(ticket_id, note, outcome, workflow_action_id, None, hidden)
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
        .create_action(ticket_id, note, "note", Some(action_id), None, false)
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

async fn exec_list_clients(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);
    let search = args.get("search").and_then(|v| v.as_str());

    let (clients, total) = client.list_clients(page, page_size, search).await?;

    Ok(serde_json::to_string_pretty(&json!({
        "clients": clients,
        "total_count": total,
        "page": page,
        "page_size": page_size,
    }))
    .unwrap())
}

async fn exec_search_clients(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or("query is required")?;
    let page_size = args
        .get("page_size")
        .and_then(|v| v.as_i64())
        .unwrap_or(20);

    let (clients, total) = client.search_clients(query, page_size).await?;

    Ok(serde_json::to_string_pretty(&json!({
        "results": clients,
        "total_count": total,
    }))
    .unwrap())
}

async fn exec_list_users(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);
    let client_id = args.get("client_id").and_then(|v| v.as_i64());
    let search = args.get("search").and_then(|v| v.as_str());

    let (users, total) = client.list_users(page, page_size, client_id, search).await?;

    Ok(serde_json::to_string_pretty(&json!({
        "users": users,
        "total_count": total,
        "page": page,
        "page_size": page_size,
    }))
    .unwrap())
}

async fn exec_get_user(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let user_id = args
        .get("user_id")
        .and_then(|v| v.as_i64())
        .ok_or("user_id is required")?;

    let result = client.get_user(user_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_search_users(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or("query is required")?;
    let page_size = args
        .get("page_size")
        .and_then(|v| v.as_i64())
        .unwrap_or(20);

    let (users, total) = client.search_users(query, page_size).await?;

    Ok(serde_json::to_string_pretty(&json!({
        "results": users,
        "total_count": total,
    }))
    .unwrap())
}

async fn exec_list_reports(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);
    let reportgroup_id = args.get("reportgroup_id").and_then(|v| v.as_i64()).unwrap_or(0);
    let search = args.get("search").and_then(|v| v.as_str());

    let (reports, total) = client.list_reports(page, page_size, reportgroup_id, search).await?;

    Ok(serde_json::to_string_pretty(&json!({
        "reports": reports,
        "total_count": total,
        "page": page,
        "page_size": page_size,
        "reportgroup_id": reportgroup_id,
    }))
    .unwrap())
}

async fn exec_list_report_categories(client: &HaloPSAClient) -> Result<String, String> {
    let categories = client.list_report_categories().await?;
    Ok(serde_json::to_string_pretty(&json!({ "categories": categories })).unwrap())
}

async fn exec_get_lookup_values(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let lookupid = args
        .get("lookupid")
        .and_then(|v| v.as_i64())
        .ok_or("lookupid is required")?;
    let istree = args.get("istree").and_then(|v| v.as_bool()).unwrap_or(true);

    let values = client.get_lookup_values(lookupid, istree).await?;
    Ok(serde_json::to_string_pretty(&json!({ "values": values })).unwrap())
}

async fn exec_get_agent(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let agent_id = args
        .get("agent_id")
        .and_then(|v| v.as_i64())
        .ok_or("agent_id is required")?;

    let result = client.get_agent(agent_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_workflow_steps(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let workflow_id = args
        .get("workflow_id")
        .and_then(|v| v.as_i64())
        .ok_or("workflow_id is required")?;

    let steps = client.list_workflow_steps(workflow_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "steps": steps })).unwrap())
}

async fn exec_run_report(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let report_id = args
        .get("report_id")
        .and_then(|v| v.as_i64())
        .ok_or("report_id is required")?;

    let mut filters: Vec<Value> = Vec::new();
    if let Some(arr) = args.get("filters").and_then(|v| v.as_array()) {
        for f in arr {
            let fieldname = f
                .get("fieldname")
                .and_then(|v| v.as_str())
                .ok_or("filters[].fieldname is required")?;
            let stringruletype = f
                .get("stringruletype")
                .and_then(|v| v.as_i64())
                .ok_or("filters[].stringruletype is required")?;
            let values: Vec<Value> = f
                .get("stringrulevalues")
                .and_then(|v| v.as_array())
                .ok_or("filters[].stringrulevalues is required")?
                .iter()
                .filter_map(|v| v.as_str())
                .map(|v| json!({ "value": v, "label": v }))
                .collect();
            filters.push(json!({
                "fieldname": fieldname,
                "stringruletype": stringruletype,
                "stringrulevalues": values,
            }));
        }
    }

    let mut params: Vec<(String, String)> = Vec::new();
    if let Some(obj) = args.get("parameters").and_then(|v| v.as_object()) {
        for (k, v) in obj {
            let val = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            params.push((k.clone(), val));
        }
    }

    let result = client.run_report(report_id, &filters, &params).await?;

    let rows = result
        .get("report")
        .and_then(|r| r.get("rows"))
        .cloned()
        .unwrap_or_else(|| json!([]));
    let row_count = rows.as_array().map(|a| a.len()).unwrap_or(0);

    Ok(serde_json::to_string_pretty(&json!({
        "report_id": report_id,
        "name": result.get("name"),
        "columns": result.get("available_columns"),
        "filterable_columns": result.get("filterable_columns"),
        "applied_filters": result.get("filters"),
        "row_count": row_count,
        "rows": rows,
    }))
    .unwrap())
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
    Ok(serde_json::to_string_pretty(&json!({ "assets": summarize_assets(&assets) })).unwrap())
}

async fn exec_get_asset(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let asset_id = args
        .get("asset_id")
        .and_then(|v| v.as_i64())
        .ok_or("asset_id is required")?;

    let result = client.get_asset(asset_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

fn summarize_assets(assets: &[Value]) -> Vec<Value> {
    assets
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
        .collect()
}

async fn exec_list_assets(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);
    let assettype_id = args.get("assettype_id").and_then(|v| v.as_i64());
    let search = args.get("search").and_then(|v| v.as_str());

    let (assets, total) = client.list_assets(page, page_size, assettype_id, search).await?;

    Ok(serde_json::to_string_pretty(&json!({
        "assets": summarize_assets(&assets),
        "total_count": total,
        "page": page,
        "page_size": page_size,
    }))
    .unwrap())
}

async fn exec_search_assets(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or("query is required")?;
    let page_size = args
        .get("page_size")
        .and_then(|v| v.as_i64())
        .unwrap_or(20);

    let (assets, total) = client.search_assets(query, page_size).await?;

    Ok(serde_json::to_string_pretty(&json!({
        "results": summarize_assets(&assets),
        "total_count": total,
    }))
    .unwrap())
}

async fn exec_list_sites(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);

    let (sites, total) = client.list_sites(page, page_size).await?;

    Ok(serde_json::to_string_pretty(&json!({
        "sites": sites,
        "total_count": total,
        "page": page,
        "page_size": page_size,
    }))
    .unwrap())
}

async fn exec_get_site(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let site_id = args
        .get("site_id")
        .and_then(|v| v.as_i64())
        .ok_or("site_id is required")?;

    let result = client.get_site(site_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

fn build_site_address(args: &Value) -> Option<Value> {
    let mapping = [
        ("address_line1", "line1"),
        ("address_line2", "line2"),
        ("address_line3", "line3"),
        ("address_line4", "line4"),
        ("address_postcode", "postcode"),
    ];
    let mut addr = json!({});
    let mut any = false;
    for (arg_key, halo_key) in mapping {
        if let Some(val) = args.get(arg_key) {
            addr[halo_key] = val.clone();
            any = true;
        }
    }
    if any { Some(addr) } else { None }
}

async fn exec_create_site(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let client_id = args
        .get("client_id")
        .and_then(|v| v.as_i64())
        .ok_or("client_id is required")?;
    let name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or("name is required")?;

    let mut site = json!({
        "client_id": client_id,
        "name": name,
    });
    for field in &["sla_id", "timezone", "inactive"] {
        if let Some(val) = args.get(*field) {
            site[*field] = val.clone();
        }
    }
    if let Some(addr) = build_site_address(args) {
        site["delivery_address"] = addr;
    }

    let result = client.create_site(site).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_update_site(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let site_id = args
        .get("site_id")
        .and_then(|v| v.as_i64())
        .ok_or("site_id is required")?;

    let mut fields = json!({});
    for field in &["name", "sla_id", "timezone", "inactive"] {
        if let Some(val) = args.get(*field) {
            fields[*field] = val.clone();
        }
    }
    if let Some(addr) = build_site_address(args) {
        fields["delivery_address"] = addr;
    }

    let result = client.update_site(site_id, fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_agents(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);

    let (agents, total) = client.list_agents(page, page_size).await?;

    Ok(serde_json::to_string_pretty(&json!({
        "agents": agents,
        "total_count": total,
        "page": page,
        "page_size": page_size,
    }))
    .unwrap())
}

async fn exec_list_asset_groups(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);

    let (groups, total) = client.list_asset_groups(page, page_size).await?;

    Ok(serde_json::to_string_pretty(&json!({
        "asset_groups": groups,
        "total_count": total,
        "page": page,
        "page_size": page_size,
    }))
    .unwrap())
}

async fn exec_get_asset_group(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let asset_group_id = args
        .get("asset_group_id")
        .and_then(|v| v.as_i64())
        .ok_or("asset_group_id is required")?;

    let result = client.get_asset_group(asset_group_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_slas(client: &HaloPSAClient) -> Result<String, String> {
    let slas = client.list_slas().await?;
    Ok(serde_json::to_string_pretty(&json!({ "slas": slas })).unwrap())
}

async fn exec_get_sla(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let sla_id = args
        .get("sla_id")
        .and_then(|v| v.as_i64())
        .ok_or("sla_id is required")?;

    let result = client.get_sla(sla_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_outcomes(client: &HaloPSAClient) -> Result<String, String> {
    let outcomes = client.list_outcomes().await?;
    let summary: Vec<Value> = outcomes
        .iter()
        .map(|o| {
            json!({
                "id": o.get("id"),
                "outcome": o.get("outcome"),
                "labellong": o.get("labellong"),
                "hidden": o.get("hidden"),
                "sequence": o.get("sequence"),
            })
        })
        .collect();
    Ok(serde_json::to_string_pretty(&json!({ "outcomes": summary })).unwrap())
}

async fn exec_list_categories(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let type_id = args
        .get("type_id")
        .and_then(|v| v.as_i64())
        .ok_or("type_id is required")?;

    let categories = client.list_categories(type_id).await?;
    let summary: Vec<Value> = categories
        .iter()
        .map(|c| {
            json!({
                "id": c.get("id"),
                "value": c.get("value"),
                "type_id": c.get("type_id"),
                "priority_id": c.get("priority_id"),
                "sla_id": c.get("sla_id"),
                "chargerate": c.get("chargerate"),
            })
        })
        .collect();
    Ok(serde_json::to_string_pretty(&json!({ "categories": summary })).unwrap())
}

async fn exec_log_time(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let ticket_id = args
        .get("ticket_id")
        .and_then(|v| v.as_i64())
        .ok_or("ticket_id is required")?;
    let time_minutes = args
        .get("time_minutes")
        .and_then(|v| v.as_f64())
        .ok_or("time_minutes is required")?;
    let note = args
        .get("note")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let outcome = args
        .get("outcome")
        .and_then(|v| v.as_str())
        .unwrap_or("note");
    let hidden = args
        .get("hidden_from_user")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let result = client.log_time(ticket_id, time_minutes, note, outcome, hidden).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_update_action(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let ticket_id = args
        .get("ticket_id")
        .and_then(|v| v.as_i64())
        .ok_or("ticket_id is required")?;
    let action_id = args
        .get("action_id")
        .and_then(|v| v.as_i64())
        .ok_or("action_id is required")?;
    let note = args
        .get("note")
        .and_then(|v| v.as_str())
        .ok_or("note is required")?;
    let hidden = args.get("hidden_from_user").and_then(|v| v.as_bool());

    let result = client.update_action(ticket_id, action_id, note, hidden).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_delete_action(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let ticket_id = args
        .get("ticket_id")
        .and_then(|v| v.as_i64())
        .ok_or("ticket_id is required")?;
    let action_id = args
        .get("action_id")
        .and_then(|v| v.as_i64())
        .ok_or("action_id is required")?;

    client.delete_action(ticket_id, action_id).await?;
    Ok(serde_json::to_string_pretty(&json!({
        "deleted": true,
        "action_id": action_id
    }))
    .unwrap())
}

async fn exec_get_action(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let ticket_id = args
        .get("ticket_id")
        .and_then(|v| v.as_i64())
        .ok_or("ticket_id is required")?;
    let action_id = args
        .get("action_id")
        .and_then(|v| v.as_i64())
        .ok_or("action_id is required")?;

    let result = client.get_action(ticket_id, action_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_send_email_reply(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let ticket_id = args
        .get("ticket_id")
        .and_then(|v| v.as_i64())
        .ok_or("ticket_id is required")?;
    let message = args
        .get("message")
        .and_then(|v| v.as_str())
        .ok_or("message is required")?;

    let result = client
        .create_action(ticket_id, message, "reply", None, None, false)
        .await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_add_internal_note(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let ticket_id = args
        .get("ticket_id")
        .and_then(|v| v.as_i64())
        .ok_or("ticket_id is required")?;
    let note = args
        .get("note")
        .and_then(|v| v.as_str())
        .ok_or("note is required")?;

    let result = client
        .create_action(ticket_id, note, "note", None, None, true)
        .await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_change_ticket_status_with_note(
    args: &Value,
    client: &HaloPSAClient,
) -> Result<String, String> {
    let ticket_id = args
        .get("ticket_id")
        .and_then(|v| v.as_i64())
        .ok_or("ticket_id is required")?;
    let status_id = args
        .get("status_id")
        .and_then(|v| v.as_i64())
        .ok_or("status_id is required")?;
    let note = args
        .get("note")
        .and_then(|v| v.as_str())
        .ok_or("note is required")?;
    let hidden = args
        .get("hidden_from_user")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Status changes go through the action payload's status_id field
    // (mirroring the note+status form in HaloPSA's own agent UI), not a
    // direct ticket field update — a plain update_ticket status_id write
    // was silently ignored by HaloPSA for workflow-driven ticket types.
    let result = client
        .create_action(ticket_id, note, "note", None, Some(status_id), hidden)
        .await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
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
