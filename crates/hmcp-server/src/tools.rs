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
                    "open_only": { "type": "boolean", "description": "Only return open tickets (default false)", "default": false },
                    "priority": { "type": "string", "description": "Filter by priority label (e.g. \"P3\", \"High\"). Priority labels are environment-specific — Sandbox uses P1-P5, Production uses named levels like High/Critical/RFO. Use list_statuses or check a real ticket to find valid labels for this instance." }
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
            "name": "list_open_tickets",
            "description": "List open (unresolved) tickets. Convenience wrapper over list_tickets with open_only=true.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "list_tickets_by_client",
            "description": "List tickets for a specific client. Convenience wrapper over list_tickets with client_id set.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "client_id": { "type": "integer", "description": "The client ID" },
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 }
                },
                "required": ["client_id"]
            }
        }),
        json!({
            "name": "list_high_priority_tickets",
            "description": "List tickets at or above a set of priority labels (defaults to Production's High/Critical/RFO — override priority_labels for other environments, e.g. Sandbox uses P1-P5 labels instead). Queries each label separately and merges results, since only single-label filtering per call is confirmed.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "priority_labels": { "type": "array", "items": { "type": "string" }, "description": "Priority labels to include (default [\"High\", \"Critical\", \"RFO\"] — Production only, won't match in Sandbox)" },
                    "page_size": { "type": "integer", "description": "Max results per priority label before merging (default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "list_unassigned_tickets",
            "description": "List tickets with no agent assigned. Confirmed against the agent UI: unassigned tickets are represented by a reserved placeholder agent_id, not a null/missing value.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "list_my_tickets",
            "description": "List tickets assigned to the authenticated agent. Resolves your own agent ID via get_me.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 }
                }
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
            "description": "List available statuses with their IDs. Pass status_type \"ticket\" to scope to ticket statuses only; omit for all statuses regardless of type.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "status_type": { "type": "string", "description": "Filter by status type, e.g. \"ticket\". Omit for all statuses." }
                }
            }
        }),
        json!({
            "name": "get_status_details",
            "description": "Get full configuration details for a single status (SLA hold behavior, email settings, colour, etc). Use list_statuses to find status IDs.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "status_id": { "type": "integer", "description": "The status ID" }
                },
                "required": ["status_id"]
            }
        }),
        json!({
            "name": "global_search",
            "description": "Search across all entity types at once (tickets, clients, users, assets, etc). Good for a broad first look when you don't know what kind of record you're looking for.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "count_per_entity": { "type": "integer", "description": "Max results per entity type (default 3)", "default": 3 }
                },
                "required": ["query"]
            }
        }),
        json!({
            "name": "list_field_groups",
            "description": "List custom field groups (collections of custom fields attached to request forms, e.g. 'Laptop Request', 'Leaver Details').",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "list_invoices",
            "description": "List invoices. Returns paginated results.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "list_recurring_invoices",
            "description": "List recurring invoices (billing templates that generate invoices on a schedule). Returns paginated results.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "get_recurring_invoice",
            "description": "Get full details of a single recurring invoice by ID, including its lines and linked credit notes.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "recurring_invoice_id": { "type": "integer", "description": "The recurring invoice ID" }
                },
                "required": ["recurring_invoice_id"]
            }
        }),
        json!({
            "name": "list_charge_rates",
            "description": "List charge types (Configuration > Billing > Charge Types) used to categorize billable ticket actions, e.g. 'SD - Remote Reactive Support', 'FIELD - Travel'.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "list_billing_lines",
            "description": "List individual billing line items (flattened from invoices). Returns paginated results. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "list_teams",
            "description": "List all teams/queues the user has access to.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "list_asset_changes",
            "description": "List change history for assets (audit trail). Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "asset_id": { "type": "integer", "description": "Filter by asset ID (optional)" },
                    "count": { "type": "integer", "description": "Max results (default 200)", "default": 200 }
                }
            }
        }),
        json!({
            "name": "create_invoice",
            "description": "Create a new invoice. Reuses the same confirmed /api/Invoice base path as list_invoices/get_invoice. Confirmed against the live sandbox: invoice_date is required or HaloPSA rejects with 400 'Date Invoiced is mandatory'.",
            "inputSchema": {
                "type": "object",
                "properties": { "fields": { "type": "object", "description": "Invoice fields, e.g. { \"client_id\": 1, \"invoice_date\": \"2026-07-07T00:00:00Z\", \"lines\": [...] }. invoice_date is required." } },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "void_invoice",
            "description": "Void an existing invoice. WARNING: cannot be undone. Field name for voiding is guessed — unconfirmed against this sandbox.",
            "inputSchema": {
                "type": "object",
                "properties": { "invoice_id": { "type": "integer", "description": "The invoice ID to void" } },
                "required": ["invoice_id"]
            }
        }),
        json!({
            "name": "list_suppliers",
            "description": "List suppliers/vendors. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "count": { "type": "integer", "description": "Number of results (default 50)", "default": 50 } }
            }
        }),
        json!({
            "name": "get_supplier",
            "description": "Get full details of a single supplier by ID. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "supplier_id": { "type": "integer", "description": "The supplier ID" } },
                "required": ["supplier_id"]
            }
        }),
        json!({
            "name": "create_supplier",
            "description": "Create a new supplier/vendor record. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "fields": { "type": "object", "description": "Supplier fields, e.g. { \"name\": \"Acme Hardware Inc.\" }" } },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "create_supplier_user",
            "description": "Create a new supplier-side contact attached to a supplier. Confirmed against the live sandbox: emailaddress is required or HaloPSA rejects with 400 'Email Address is mandatory'.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "supplier_id": { "type": "integer", "description": "The supplier ID this user belongs to" },
                    "supplier_name": { "type": "string", "description": "The supplier's display name (must match the supplier record)" },
                    "fields": { "type": "object", "description": "User fields, e.g. { \"firstname\": \"Jane\", \"lastname\": \"Doe\", \"emailaddress\": \"jane@acme.com\" }" }
                },
                "required": ["supplier_id", "supplier_name", "fields"]
            }
        }),
        json!({
            "name": "list_sales_orders",
            "description": "List sales orders. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "client_id": { "type": "integer", "description": "Filter by client ID (optional)" },
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "get_sales_order",
            "description": "Get full details of a single sales order by ID. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "sales_order_id": { "type": "integer", "description": "The sales order ID" } },
                "required": ["sales_order_id"]
            }
        }),
        json!({
            "name": "create_sales_order",
            "description": "Create a new sales order. Confirmed against the live sandbox: user_id (a valid contact on the client) is required or HaloPSA rejects with 400 'Please select a valid User'.",
            "inputSchema": {
                "type": "object",
                "properties": { "fields": { "type": "object", "description": "e.g. { \"client_id\": 1, \"user_id\": 42, \"lines\": [...] }. user_id is required." } },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "get_audit_entry",
            "description": "Get a single audit log entry by ID (before/after values, user, timestamp). Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "audit_entry_id": { "type": "integer", "description": "The audit entry ID" } },
                "required": ["audit_entry_id"]
            }
        }),
        json!({
            "name": "get_field",
            "description": "Get a single custom field definition by ID (name, data type, validation rules, dropdown options). Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "field_id": { "type": "integer", "description": "The field ID" } },
                "required": ["field_id"]
            }
        }),
        json!({
            "name": "get_pax8_data",
            "description": "Retrieve Pax8 distributor data (subscriptions, products, companies) from a connected Pax8 integration. Endpoint guessed AND depends on integration authorization — check list_integration_configs first.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "datatype": { "type": "string", "description": "Data category, e.g. subscriptions, products, companies. Required — this endpoint 400s if omitted." },
                    "search": { "type": "string", "description": "Free-text search (optional)" }
                },
                "required": ["datatype"]
            }
        }),
        json!({
            "name": "get_meraki_data",
            "description": "Retrieve Meraki network data from a connected Meraki integration. Endpoint guessed AND depends on integration authorization — check list_integration_configs first.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "import_from_xero",
            "description": "Trigger a data import sync from the connected Xero integration. WARNING: irreversibly syncs external data. Endpoint guessed — unconfirmed against this sandbox, never live-tested.",
            "inputSchema": {
                "type": "object",
                "properties": { "options": { "type": "object", "description": "Import options, if any" } }
            }
        }),
        json!({
            "name": "list_approval_processes",
            "description": "List approval processes (multi-step approval workflows for changes, purchases, etc). Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "get_approval_process",
            "description": "Get full details of a single approval process by ID. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "process_id": { "type": "integer", "description": "The approval process ID" } },
                "required": ["process_id"]
            }
        }),
        json!({
            "name": "create_approval_process",
            "description": "Create a new approval process. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "fields": { "type": "object", "description": "e.g. { \"name\": \"Change Approval\", \"type\": 1 }" } },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "list_approval_rules",
            "description": "List approval rules that define when approvals are triggered. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "process_id": { "type": "integer", "description": "Filter by approval process ID (optional)" } }
            }
        }),
        json!({
            "name": "create_approval_rule",
            "description": "Create a new approval rule. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "fields": { "type": "object", "description": "e.g. { \"process_id\": 1, \"name\": \"High Priority Changes\" }" } },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "list_views",
            "description": "List saved views (pre-configured filter/column layouts) for a domain, e.g. tickets or opportunities. Confirmed against the agent UI's own request.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "domain": { "type": "string", "description": "Entity domain, e.g. 'reqs' (tickets), 'opps' (opportunities)" },
                    "view_type": { "type": "string", "description": "View type, usually matches domain" }
                },
                "required": ["domain", "view_type"]
            }
        }),
        json!({
            "name": "get_view",
            "description": "Get full details of a single saved view by ID. Endpoint guessed — unconfirmed against this sandbox (list_views itself is confirmed).",
            "inputSchema": {
                "type": "object",
                "properties": { "view_id": { "type": "integer", "description": "The view ID" } },
                "required": ["view_id"]
            }
        }),
        json!({
            "name": "list_view_filters",
            "description": "List available filter definitions for a view domain. Confirmed against the agent UI's own request.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "view_type": { "type": "string", "description": "View type, e.g. 'reqs' (tickets), 'opps' (opportunities)" },
                    "ticketarea_id": { "type": "integer", "description": "Filter by ticket area ID (optional)" }
                },
                "required": ["view_type"]
            }
        }),
        json!({
            "name": "list_view_columns",
            "description": "List available column definitions for a view type. Confirmed via sandbox capture (Edit Columns on a ticket list) — same params as list_view_filters.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "view_type": { "type": "string", "description": "View type, e.g. 'reqs' (tickets), 'opps' (opportunities)" },
                    "ticketarea_id": { "type": "integer", "description": "Filter by ticket area ID (optional)" }
                },
                "required": ["view_type"]
            }
        }),
        json!({
            "name": "create_client",
            "description": "Create a new client (customer organization). Reuses the same confirmed /api/Client base path as get_client/list_clients.",
            "inputSchema": {
                "type": "object",
                "properties": { "fields": { "type": "object", "description": "Client fields, e.g. { \"name\": \"Acme Corp\", \"website\": \"https://acme.com\" }" } },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "list_invoice_payments",
            "description": "List payments recorded against invoices. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "invoice_id": { "type": "integer", "description": "Filter by invoice ID (optional)" },
                    "count": { "type": "integer", "description": "Number of results (default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "list_workflows",
            "description": "List workflows (multi-step processes for ticket handling). Uses the same base path as the already-confirmed get_available_actions/list_workflow_steps.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "get_workflow",
            "description": "Get full details of a single workflow by ID. Uses the same confirmed endpoint as list_workflow_steps.",
            "inputSchema": {
                "type": "object",
                "properties": { "workflow_id": { "type": "integer", "description": "The workflow ID" } },
                "required": ["workflow_id"]
            }
        }),
        json!({
            "name": "create_workflow",
            "description": "Create a new workflow. Endpoint unconfirmed against this sandbox for writes — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "fields": { "type": "object", "description": "Workflow fields, e.g. { \"name\": \"New Employee Onboarding\" }" } },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "update_workflow",
            "description": "Update an existing workflow's fields. Confirmed limitation: HaloPSA validates the whole workflow structure on every update (not just the changed fields) and rejects with 400 'Please select a Start Step for this Workflow' unless a fully valid start step/stages/steps structure is present — a simple field-only partial update (even re-sending the existing stages array) is not enough. Fetch the full structure with get_workflow first if editing an existing workflow's stages/steps.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workflow_id": { "type": "integer", "description": "The workflow ID" },
                    "fields": { "type": "object", "description": "Fields to update" }
                },
                "required": ["workflow_id", "fields"]
            }
        }),
        json!({
            "name": "delete_workflow",
            "description": "Delete a workflow by ID. WARNING: permanently removes it; active tickets using it may be affected. Endpoint unconfirmed against this sandbox.",
            "inputSchema": {
                "type": "object",
                "properties": { "workflow_id": { "type": "integer", "description": "The workflow ID" } },
                "required": ["workflow_id"]
            }
        }),
        json!({
            "name": "list_notifications",
            "description": "List notification rule definitions. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "agent_id": { "type": "integer", "description": "Filter by agent ID (optional)" },
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "get_notification",
            "description": "Get a single notification rule by ID. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "notification_id": { "type": "integer", "description": "The notification ID" } },
                "required": ["notification_id"]
            }
        }),
        json!({
            "name": "create_notification",
            "description": "Create a new notification rule. Confirmed against the live sandbox: the rule's title field is `name` (not `subject`), and `eventno` (an integer trigger-event enum, e.g. 1 = New Ticket Logged) is required — HaloPSA rejects the request with 'Please select an event to trigger the Notification' without it. Use get_notification/list_notifications on an existing rule with the desired trigger to find the eventno value, since the enum isn't otherwise documented.",
            "inputSchema": {
                "type": "object",
                "properties": { "fields": { "type": "object", "description": "Notification fields, e.g. { \"name\": \"VIP escalation\", \"eventno\": 1, \"agent_id\": 12 }" } },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "send_notification_message",
            "description": "Send a direct notification message to an agent. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "fields": { "type": "object", "description": "e.g. { \"message\": \"Please review ticket #5678\", \"agent_id\": 12 }" } },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "list_services",
            "description": "List services in the service catalogue. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "search": { "type": "string", "description": "Free-text search (optional)" },
                    "category_id": { "type": "integer", "description": "Filter by service category ID (optional)" },
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "get_service",
            "description": "Get full details of a single service by ID. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "service_id": { "type": "integer", "description": "The service ID" } },
                "required": ["service_id"]
            }
        }),
        json!({
            "name": "list_service_categories",
            "description": "List service catalogue categories. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "list_service_statuses",
            "description": "List service status history, optionally for a specific service. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "service_id": { "type": "integer", "description": "Filter by service ID (optional)" } }
            }
        }),
        json!({
            "name": "create_service_status",
            "description": "Record a new service status entry (operational, degraded, outage). Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "fields": { "type": "object", "description": "e.g. { \"service_id\": 1, \"status\": \"degraded\" }" } },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "list_invoice_statuses",
            "description": "List invoice statuses (e.g. Draft, Approved, Posted, Paid). Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "get_invoice_status",
            "description": "Get a single invoice status by ID. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "status_id": { "type": "integer", "description": "The invoice status ID" } },
                "required": ["status_id"]
            }
        }),
        json!({
            "name": "create_invoice_status",
            "description": "Create a new invoice status. Confirmed against the live sandbox: the name field is status_name (not name), and type (an int stage enum: 0=Draft-like, 1=Awaiting Approval, 2=Approved, 3=Posted, 4=Sent, 5=Paid, 6=Voided, 7=Closed) is required — omitting it triggers a raw SQL NOT NULL error from HaloPSA (500-shaped 400: \"Cannot insert the value NULL into column 'ISType'\").",
            "inputSchema": {
                "type": "object",
                "properties": { "fields": { "type": "object", "description": "e.g. { \"status_name\": \"Awaiting Payment\", \"type\": 1 }. Both fields are required." } },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "delete_invoice_status",
            "description": "Delete an invoice status by ID. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "status_id": { "type": "integer", "description": "The invoice status ID" } },
                "required": ["status_id"]
            }
        }),
        json!({
            "name": "update_invoice_lines",
            "description": "Bulk-update invoice line items. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "lines": { "type": "array", "description": "Line objects, each including its id and the fields to change", "items": { "type": "object" } } },
                "required": ["lines"]
            }
        }),
        json!({
            "name": "update_sales_order_lines",
            "description": "Bulk-update sales order line items. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "lines": { "type": "array", "description": "Line objects, each including its id and the fields to change", "items": { "type": "object" } } },
                "required": ["lines"]
            }
        }),
        json!({
            "name": "register_invoice_view",
            "description": "Record that a user has viewed an invoice. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "invoice_id": { "type": "integer", "description": "The invoice ID" } },
                "required": ["invoice_id"]
            }
        }),
        json!({
            "name": "register_sales_order_view",
            "description": "Record that a user has viewed a sales order. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "sales_order_id": { "type": "integer", "description": "The sales order ID" } },
                "required": ["sales_order_id"]
            }
        }),
        json!({
            "name": "register_purchase_order_view",
            "description": "Record that a user has viewed a purchase order. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "purchase_order_id": { "type": "integer", "description": "The purchase order ID" } },
                "required": ["purchase_order_id"]
            }
        }),
        json!({
            "name": "register_kb_article_view",
            "description": "Record that a user has viewed a knowledge base article. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "kb_article_id": { "type": "integer", "description": "The KB article ID" } },
                "required": ["kb_article_id"]
            }
        }),
        json!({
            "name": "review_expense",
            "description": "Mark one or more expenses as reviewed. Financial approval action. Endpoint and body shape guessed — unconfirmed against this sandbox, never live-tested.",
            "inputSchema": {
                "type": "object",
                "properties": { "expense_ids": { "type": "array", "items": { "type": "integer" }, "description": "Expense IDs to mark reviewed" } },
                "required": ["expense_ids"]
            }
        }),
        json!({
            "name": "expire_client_prepay",
            "description": "Expire one or more client prepay balances. WARNING: voids remaining prepaid credit. Endpoint and body shape guessed — unconfirmed against this sandbox, never live-tested.",
            "inputSchema": {
                "type": "object",
                "properties": { "prepay_ids": { "type": "array", "items": { "type": "integer" }, "description": "Prepay record IDs to expire" } },
                "required": ["prepay_ids"]
            }
        }),
        json!({
            "name": "list_integration_configs",
            "description": "List all configured third-party integrations with their connection status. Endpoint path unconfirmed against this sandbox (real field shape confirmed via StackJack reference).",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "get_integration_config",
            "description": "Get full details of a single integration config by ID. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "config_id": { "type": "integer", "description": "The integration config ID" } },
                "required": ["config_id"]
            }
        }),
        json!({
            "name": "list_integration_site_mappings",
            "description": "List site mappings between Halo and a connected third-party integration. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "module_id": { "type": "integer", "description": "Filter by integration module ID (optional)" } }
            }
        }),
        json!({
            "name": "list_integration_errors",
            "description": "List integration sync errors, for troubleshooting failed third-party syncs. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "module_id": { "type": "integer", "description": "Filter by integration module ID (optional)" },
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-200, default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "get_integration_error",
            "description": "Get full details of a single integration error by ID. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "error_id": { "type": "integer", "description": "The integration error ID" } },
                "required": ["error_id"]
            }
        }),
        json!({
            "name": "list_integration_requests",
            "description": "List integration API request/response logs. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "module_id": { "type": "integer", "description": "Filter by integration module ID (optional)" },
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-200, default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "get_integration_request",
            "description": "Get full details of a single integration request log entry by ID. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "request_id": { "type": "integer", "description": "The integration request ID" } },
                "required": ["request_id"]
            }
        }),
        json!({
            "name": "list_integration_field_mappings",
            "description": "List field mappings between Halo and a connected third-party integration. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "module_id": { "type": "integer", "description": "Filter by integration module ID (optional)" } }
            }
        }),
        json!({
            "name": "get_microsoft_csp_data",
            "description": "Retrieve Microsoft CSP data (subscriptions, licences, tenants) from a connected Microsoft Partner Center integration. Highest-uncertainty tool in this batch — endpoint guessed AND depends on whether this integration is actually authorized on the target tenant (check list_integration_configs first).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "datatype": { "type": "string", "description": "Data category, e.g. subscriptions, licences, customers. Required — this endpoint 400s if omitted." },
                    "search": { "type": "string", "description": "Free-text search (optional)" }
                },
                "required": ["datatype"]
            }
        }),
        json!({
            "name": "get_intune_data",
            "description": "Retrieve Microsoft Intune data (devices, compliance policies, apps) from a connected Intune integration. Endpoint guessed AND depends on integration authorization — check list_integration_configs first.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "datatype": { "type": "string", "description": "Data category, e.g. devices, compliancepolicies, apps. Required — this endpoint 400s if omitted." },
                    "search": { "type": "string", "description": "Free-text search (optional)" }
                },
                "required": ["datatype"]
            }
        }),
        json!({
            "name": "get_azure_ad_data",
            "description": "Retrieve Azure AD/Entra ID data (users, groups, licences) from a connected Azure AD integration. Endpoint guessed AND depends on integration authorization — check list_integration_configs first.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "datatype": { "type": "string", "description": "Data category, e.g. users, groups, licenses. Required — this endpoint 400s if omitted." },
                    "search": { "type": "string", "description": "Free-text search (optional)" }
                },
                "required": ["datatype"]
            }
        }),
        json!({
            "name": "get_ninja_rmm_data",
            "description": "Retrieve NinjaOne (formerly NinjaRMM) data from a connected NinjaOne integration — confirmed active in this sandbox via Config > Integrations.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "datatype": { "type": "string", "description": "Data category, e.g. devices, organizations. Required — this endpoint 400s if omitted." },
                    "search": { "type": "string", "description": "Free-text search (optional)" }
                },
                "required": ["datatype"]
            }
        }),
        json!({
            "name": "get_xero_data",
            "description": "Retrieve Xero accounting data (invoices, contacts, payments) from a connected Xero integration. Endpoint guessed AND depends on integration authorization — check list_integration_configs first.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "datatype": { "type": "string", "description": "Data category, e.g. invoices, contacts, payments. Required — this endpoint 400s if omitted." },
                    "search": { "type": "string", "description": "Free-text search (optional)" }
                },
                "required": ["datatype"]
            }
        }),
        json!({
            "name": "list_xero_details",
            "description": "List Xero sync detail records. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "get_xero_detail",
            "description": "Get a single Xero sync detail record by ID. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "detail_id": { "type": "integer", "description": "The Xero detail record ID" } },
                "required": ["detail_id"]
            }
        }),
        json!({
            "name": "send_invoice_to_xero",
            "description": "Push an invoice to the connected Xero accounting integration. Endpoint guessed — unconfirmed against this sandbox, never live-tested.",
            "inputSchema": {
                "type": "object",
                "properties": { "invoice_id": { "type": "integer", "description": "The invoice ID to send" } },
                "required": ["invoice_id"]
            }
        }),
        json!({
            "name": "create_asset",
            "description": "Create a new asset. Reuses the same confirmed /api/Asset base path as get_asset/list_assets.",
            "inputSchema": {
                "type": "object",
                "properties": { "fields": { "type": "object", "description": "Asset fields, e.g. { \"inventory_number\": \"PC-001\", \"client_id\": 42, \"assettype_id\": 1 }" } },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "update_asset",
            "description": "Update an existing asset's fields. Reuses the same confirmed /api/Asset base path as get_asset/list_assets.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "asset_id": { "type": "integer", "description": "The asset ID" },
                    "fields": { "type": "object", "description": "Fields to update" }
                },
                "required": ["asset_id", "fields"]
            }
        }),
        json!({
            "name": "list_asset_software",
            "description": "List software discovered on a specific asset. Confirmed via sandbox capture — there's no standalone software resource, this reads the software inventory embedded in the asset's own record.",
            "inputSchema": {
                "type": "object",
                "properties": { "asset_id": { "type": "integer", "description": "The asset/device ID" } },
                "required": ["asset_id"]
            }
        }),
        json!({
            "name": "list_roles",
            "description": "List access-control roles. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "get_role",
            "description": "Get full details of a single role by ID. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "role_id": { "type": "integer", "description": "The role ID" } },
                "required": ["role_id"]
            }
        }),
        json!({
            "name": "create_role",
            "description": "Create a new access-control role. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "fields": { "type": "object", "description": "Role fields, e.g. { \"name\": \"L2 Support\" }" } },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "update_role",
            "description": "Update an existing role's fields. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "role_id": { "type": "integer", "description": "The role ID" },
                    "fields": { "type": "object", "description": "Fields to update" }
                },
                "required": ["role_id", "fields"]
            }
        }),
        json!({
            "name": "delete_role",
            "description": "Delete a role by ID. WARNING: permanently removes it; agents assigned this role may lose permissions. Endpoint unconfirmed against this sandbox.",
            "inputSchema": {
                "type": "object",
                "properties": { "role_id": { "type": "integer", "description": "The role ID" } },
                "required": ["role_id"]
            }
        }),
        json!({
            "name": "list_tags",
            "description": "List tags used to categorize tickets, assets, clients, etc.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "search": { "type": "string", "description": "Search tags by name (optional)" },
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "get_tag",
            "description": "Get a single tag by ID. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "tag_id": { "type": "integer", "description": "The tag ID" } },
                "required": ["tag_id"]
            }
        }),
        json!({
            "name": "create_tag",
            "description": "Create a new tag. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "fields": { "type": "object", "description": "Tag fields, e.g. { \"text\": \"VIP\" }" } },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "delete_tag",
            "description": "Delete a tag by ID. WARNING: permanently removes it and detaches it from all records. Endpoint unconfirmed against this sandbox.",
            "inputSchema": {
                "type": "object",
                "properties": { "tag_id": { "type": "integer", "description": "The tag ID" } },
                "required": ["tag_id"]
            }
        }),
        json!({
            "name": "list_item_groups",
            "description": "List item groups/categories used to organise the product catalogue. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-200, default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "get_item_group",
            "description": "Get a single item group by ID. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "group_id": { "type": "integer", "description": "The item group ID" } },
                "required": ["group_id"]
            }
        }),
        json!({
            "name": "create_item_group",
            "description": "Create a new item group. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "fields": { "type": "object", "description": "Group fields, e.g. { \"name\": \"Networking Equipment\" }" } },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "delete_item_group",
            "description": "Delete an item group by ID. Items in the group are not deleted, only ungrouped. Endpoint unconfirmed against this sandbox.",
            "inputSchema": {
                "type": "object",
                "properties": { "group_id": { "type": "integer", "description": "The item group ID" } },
                "required": ["group_id"]
            }
        }),
        json!({
            "name": "list_items",
            "description": "List catalogue items (products/parts used in quotations, invoices, purchase orders). Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "search": { "type": "string", "description": "Search by name or description (optional)" },
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-200, default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "get_item",
            "description": "Get a single catalogue item by ID. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "item_id": { "type": "integer", "description": "The item ID" } },
                "required": ["item_id"]
            }
        }),
        json!({
            "name": "create_item",
            "description": "Create a new catalogue item. Confirmed against the live sandbox and the Halo API docs (/apidoc/resources/items): requires assetgroup_id (an Item Group's ID, from create_item_group/list_item_groups — despite the name, it is not an Asset Group), and the price field is baseprice, not unit_price.",
            "inputSchema": {
                "type": "object",
                "properties": { "fields": { "type": "object", "description": "Item fields, e.g. { \"name\": \"USB-C Hub\", \"assetgroup_id\": 27, \"baseprice\": 49.99 }. assetgroup_id is required (HaloPSA returns 'Item group must be completed' without it)." } },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "list_item_stock",
            "description": "List stock levels for catalogue items. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "item_id": { "type": "integer", "description": "Filter by item ID (optional)" } }
            }
        }),
        json!({
            "name": "list_products",
            "description": "List products (sellable bundles). Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "search": { "type": "string", "description": "Search by name or description (optional)" },
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "get_product",
            "description": "Get a single product by ID. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "product_id": { "type": "integer", "description": "The product ID" } },
                "required": ["product_id"]
            }
        }),
        json!({
            "name": "create_product",
            "description": "Create a new product. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "fields": { "type": "object", "description": "Product fields, e.g. { \"name\": \"Standard Laptop Bundle\" }" } },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "delete_product",
            "description": "Delete a product by ID. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "product_id": { "type": "integer", "description": "The product ID" } },
                "required": ["product_id"]
            }
        }),
        json!({
            "name": "list_product_components",
            "description": "List the components (items) that make up a product bundle. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "product_id": { "type": "integer", "description": "Filter by product ID (optional)" },
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "create_product_component",
            "description": "Add a component (item) to a product bundle. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "fields": { "type": "object", "description": "e.g. { \"product_id\": 5, \"item_id\": 12, \"quantity\": 1 }" } },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "list_quotations",
            "description": "List quotations/quotes. Returns paginated results. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "client_id": { "type": "integer", "description": "Filter by client ID (optional)" },
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-200, default 50)", "default": 50 }
                }
            }
        }),
        json!({
            "name": "get_quotation",
            "description": "Get full details of a single quotation by ID, including line items. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "quotation_id": { "type": "integer", "description": "The quotation ID" } },
                "required": ["quotation_id"]
            }
        }),
        json!({
            "name": "create_quotation",
            "description": "Create a new quotation. Confirmed against the live sandbox: user_id (a valid contact on the client, from list_users) is required or HaloPSA rejects with 400 'Please select a valid User'.",
            "inputSchema": {
                "type": "object",
                "properties": { "fields": { "type": "object", "description": "Quotation fields, e.g. { \"client_id\": 1, \"title\": \"Office Refresh\", \"user_id\": 42 }. user_id is required." } },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "update_quotation_lines",
            "description": "Update line items on an existing quotation. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "quotation_id": { "type": "integer", "description": "The quotation ID" },
                    "lines": { "type": "array", "description": "Updated line items", "items": { "type": "object" } }
                },
                "required": ["quotation_id", "lines"]
            }
        }),
        json!({
            "name": "approve_quotation",
            "description": "Approve or reject a quotation. May trigger downstream invoice/sales order creation. Endpoint unconfirmed against this sandbox.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "quotation_id": { "type": "integer", "description": "The quotation ID" },
                    "approved": { "type": "boolean", "description": "true to approve, false to reject" },
                    "notes": { "type": "string", "description": "Optional approval note" }
                },
                "required": ["quotation_id", "approved"]
            }
        }),
        json!({
            "name": "view_quotation",
            "description": "Get a presentation-ready view of a quotation (same data as get_quotation — no distinct rendered-view endpoint was found).",
            "inputSchema": {
                "type": "object",
                "properties": { "quotation_id": { "type": "integer", "description": "The quotation ID" } },
                "required": ["quotation_id"]
            }
        }),
        json!({
            "name": "list_timesheets",
            "description": "List timesheet entries across agents. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "agent_id": { "type": "integer", "description": "Filter by agent ID (optional)" } }
            }
        }),
        json!({
            "name": "get_my_timesheets",
            "description": "Get timesheet entries for the currently authenticated agent. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "get_timesheet",
            "description": "Get a single timesheet entry by ID. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "timesheet_id": { "type": "integer", "description": "The timesheet entry ID" } },
                "required": ["timesheet_id"]
            }
        }),
        json!({
            "name": "create_timesheet",
            "description": "Create/update a daily workday timesheet record for an agent (their scheduled vs. worked hours for one calendar day) — NOT a per-ticket time entry; use log_time for that. Confirmed against the live sandbox: the previously assumed shape ({agent_id, hours, ticket_id}) causes a 500 Internal Server Error. The real shape mirrors list_timesheets' rows: { agent_id, date, workdayid, work_hours }.",
            "inputSchema": {
                "type": "object",
                "properties": { "fields": { "type": "object", "description": "e.g. { \"agent_id\": 5, \"date\": \"2026-07-07T00:00:00Z\", \"workdayid\": 1, \"work_hours\": 1.5 }" } },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "get_invoice",
            "description": "Get full details of a single invoice by ID, including line items.",
            "inputSchema": {
                "type": "object",
                "properties": { "invoice_id": { "type": "integer", "description": "The invoice ID" } },
                "required": ["invoice_id"]
            }
        }),
        json!({
            "name": "list_invoice_lines",
            "description": "List invoice line items for a specific invoice, or across recent invoices if no invoice_id given.",
            "inputSchema": {
                "type": "object",
                "properties": { "invoice_id": { "type": "integer", "description": "Filter to a specific invoice (optional)" } }
            }
        }),
        json!({
            "name": "create_asset_group",
            "description": "Create a new asset group. Confirmed against the live sandbox: HaloPSA rejects the request with 'Please pick use for this group' unless at least one of showasequip/showasitem is set true — it is not a \"use\" field despite the error text.",
            "inputSchema": {
                "type": "object",
                "properties": { "fields": { "type": "object", "description": "Group fields, e.g. { \"name\": \"Server Room A\", \"showasequip\": true }. Set showasequip and/or showasitem true depending on whether the group is for assets, items, or both." } },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "update_asset_group",
            "description": "Update an existing asset group's fields. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "group_id": { "type": "integer", "description": "The asset group ID" },
                    "fields": { "type": "object", "description": "Fields to update" }
                },
                "required": ["group_id", "fields"]
            }
        }),
        json!({
            "name": "delete_asset_group",
            "description": "Delete an asset group by ID. Assets in the group are not deleted, only ungrouped. Endpoint unconfirmed against this sandbox.",
            "inputSchema": {
                "type": "object",
                "properties": { "group_id": { "type": "integer", "description": "The asset group ID" } },
                "required": ["group_id"]
            }
        }),
        json!({
            "name": "list_attachments",
            "description": "List attachment metadata, optionally scoped to a ticket. Does not fetch file bytes. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": { "ticket_id": { "type": "integer", "description": "Filter by ticket ID (optional)" } }
            }
        }),
        json!({
            "name": "get_attachment",
            "description": "Get attachment metadata by ID (filename, size, MIME type) — does not fetch file bytes. Endpoint unconfirmed against this sandbox.",
            "inputSchema": {
                "type": "object",
                "properties": { "attachment_id": { "type": "integer", "description": "The attachment ID" } },
                "required": ["attachment_id"]
            }
        }),
        json!({
            "name": "delete_attachment",
            "description": "Delete an attachment by ID. WARNING: permanently removes the file. Endpoint unconfirmed against this sandbox.",
            "inputSchema": {
                "type": "object",
                "properties": { "attachment_id": { "type": "integer", "description": "The attachment ID" } },
                "required": ["attachment_id"]
            }
        }),
        json!({
            "name": "update_client",
            "description": "Update an existing client's fields (name, contact details, custom fields).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "client_id": { "type": "integer", "description": "The client ID" },
                    "fields": { "type": "object", "description": "Fields to update, e.g. { \"name\": \"Acme Corp\" }" }
                },
                "required": ["client_id", "fields"]
            }
        }),
        json!({
            "name": "list_asset_types",
            "description": "List all available asset types configured in HaloPSA. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "search_agents",
            "description": "Search agents (technicians) by name or email. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query string" },
                    "page_size": { "type": "integer", "description": "Max results (default 50)", "default": 50 }
                },
                "required": ["query"]
            }
        }),
        json!({
            "name": "list_projects",
            "description": "List projects (Tickets scoped to the 'Projects' ticket area). Returns paginated results.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 },
                    "client_id": { "type": "integer", "description": "Filter by client ID (optional)" }
                }
            }
        }),
        json!({
            "name": "get_project",
            "description": "Get full details of a single project by ID (same underlying record as get_ticket).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "integer", "description": "The project (ticket) ID" }
                },
                "required": ["project_id"]
            }
        }),
        json!({
            "name": "create_project",
            "description": "Create a new project. Requires an appropriate tickettype_id (use list_ticket_types to find the Project-designated type) — the Projects ticketarea_id is filled in automatically.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "fields": { "type": "object", "description": "Ticket fields, e.g. { \"summary\": \"Office Migration\", \"client_id\": 42, \"tickettype_id\": 20 }" }
                },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "update_project",
            "description": "Update an existing project's fields (same underlying record as update_ticket).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "integer", "description": "The project (ticket) ID" },
                    "fields": { "type": "object", "description": "Fields to update" }
                },
                "required": ["project_id", "fields"]
            }
        }),
        json!({
            "name": "list_project_tasks",
            "description": "List tasks (child tickets) under a project. Parent/child field unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project_id": { "type": "integer", "description": "The project (ticket) ID" },
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-100, default 50)", "default": 50 }
                },
                "required": ["project_id"]
            }
        }),
        json!({
            "name": "create_report_pdf",
            "description": "Confirmed NOT to work as described: the `ispdf`/`dontloadsystemreport` query params this sends aren't real HaloPSA parameters (absent from the official API docs) and are silently ignored — this just returns the same report JSON as fetching the report normally (with loadreport=true), never a PDF. No real PDF-export endpoint has been found for reports. Prefer run_report for report data; treat any PDF/file expectation from this tool as unmet.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "report_id": { "type": "integer", "description": "The report ID" },
                    "filters": { "type": "array", "description": "Optional filter overrides, same shape as run_report's filters", "items": { "type": "object" } }
                },
                "required": ["report_id"]
            }
        }),
        json!({
            "name": "list_opportunities",
            "description": "List CRM opportunities/deals (sales pipeline). Returns paginated results.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "page": { "type": "integer", "description": "Page number (default 1)", "default": 1 },
                    "page_size": { "type": "integer", "description": "Results per page (1-200, default 50)", "default": 50 },
                    "client_id": { "type": "integer", "description": "Filter by client ID (optional)" }
                }
            }
        }),
        json!({
            "name": "get_opportunity",
            "description": "Get full details of a single CRM opportunity by ID.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "opportunity_id": { "type": "integer", "description": "The opportunity ID" }
                },
                "required": ["opportunity_id"]
            }
        }),
        json!({
            "name": "create_opportunity",
            "description": "Create a new CRM opportunity/deal. Confirmed against the live sandbox: `targetdate` is required or HaloPSA rejects with 400 'Target Date is mandatory'.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "fields": { "type": "object", "description": "Opportunity fields, e.g. { \"summary\": \"Managed Services Contract\", \"client_id\": 1, \"oppvalueadjusted\": 24000, \"targetdate\": \"2026-12-31T00:00:00Z\" }. targetdate is required." }
                },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "update_opportunity",
            "description": "Update an existing CRM opportunity's fields (stage, value, probability, etc).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "opportunity_id": { "type": "integer", "description": "The opportunity ID" },
                    "fields": { "type": "object", "description": "Fields to update" }
                },
                "required": ["opportunity_id", "fields"]
            }
        }),
        json!({
            "name": "list_crm_notes",
            "description": "List CRM notes against a client or supplier. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "client_id": { "type": "integer", "description": "Filter by client ID (optional)" },
                    "supplier_id": { "type": "integer", "description": "Filter by supplier ID (optional)" }
                }
            }
        }),
        json!({
            "name": "create_crm_note",
            "description": "Create a CRM note against a client or supplier. Confirmed against the live sandbox.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "fields": { "type": "object", "description": "Note fields, e.g. { \"client_id\": 1, \"note\": \"Discussed renewal terms\" }" }
                },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "list_contact_groups",
            "description": "List contact groups (shown in the agent UI as 'Distribution Lists').",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "manage_contact_group_members",
            "description": "Add or remove a user from a contact group (distribution list). Field shape unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "group_id": { "type": "integer", "description": "The contact group ID" },
                    "user_id": { "type": "integer", "description": "The user ID to add or remove" },
                    "add": { "type": "boolean", "description": "true to add, false to remove (default true)", "default": true }
                },
                "required": ["group_id", "user_id"]
            }
        }),
        json!({
            "name": "list_ticket_approvals",
            "description": "List pending ticket approvals.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "mine": { "type": "boolean", "description": "Only approvals awaiting your action (default true)", "default": true }
                }
            }
        }),
        json!({
            "name": "process_approval",
            "description": "Approve or reject one or more pending ticket approvals. WARNING: irreversible once processed. Body shape unconfirmed against this sandbox.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "approval_ids": { "type": "array", "items": { "type": "integer" }, "description": "Approval IDs to process" },
                    "approve": { "type": "boolean", "description": "true to approve, false to reject" }
                },
                "required": ["approval_ids", "approve"]
            }
        }),
        json!({
            "name": "list_feedback",
            "description": "List customer satisfaction (CSAT) feedback entries. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "client_id": { "type": "integer", "description": "Filter by client ID (optional)" },
                    "agent_id": { "type": "integer", "description": "Filter by agent ID (optional)" }
                }
            }
        }),
        json!({
            "name": "list_custom_tables",
            "description": "List custom data tables used for extending Halo's data model. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "get_custom_table",
            "description": "Get a single custom table by ID, including column definitions. Endpoint unconfirmed against this sandbox — flag results as unverified.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "table_id": { "type": "integer", "description": "The custom table ID" }
                },
                "required": ["table_id"]
            }
        }),
        json!({
            "name": "create_custom_table",
            "description": "Create a new custom data table. Confirmed against the live sandbox: `name` becomes a real backing database table name (prefixed CT), so it must be alphanumeric with no spaces or special characters — HaloPSA rejects e.g. \"Vendor Certifications\" with 400 'Invalid Name' but accepts \"VendorCertifications\".",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "fields": { "type": "object", "description": "Table fields, e.g. { \"name\": \"VendorCertifications\" }. name must be alphanumeric only (no spaces/punctuation)." }
                },
                "required": ["fields"]
            }
        }),
        json!({
            "name": "delete_custom_table",
            "description": "Delete a custom table by ID. WARNING: permanently removes the table and all its data. Endpoint unconfirmed against this sandbox.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "table_id": { "type": "integer", "description": "The custom table ID" }
                },
                "required": ["table_id"]
            }
        }),
        json!({
            "name": "list_ticket_types",
            "description": "List all available ticket types.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "get_ticket_type_details",
            "description": "Get full configuration details for a single ticket type, including its workflow, categories, and other settings.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tickettype_id": { "type": "integer", "description": "The ticket type ID (from list_ticket_types)" }
                },
                "required": ["tickettype_id"]
            }
        }),
        json!({
            "name": "list_ticket_type_fields",
            "description": "List the fields configured for a ticket type (e.g. Summary, Category, Priority, plus any custom fields).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tickettype_id": { "type": "integer", "description": "The ticket type ID (from list_ticket_types)" }
                },
                "required": ["tickettype_id"]
            }
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
            "name": "list_ticket_areas",
            "description": "List ticket areas (e.g. Service Desk, Projects, Internal Processes) — the area a ticket belongs to.",
            "inputSchema": { "type": "object", "properties": {} }
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
            "name": "list_priorities",
            "description": "List the priority levels defined on an SLA (e.g. P1 Site Down, P2 Mission Critical). Use list_slas to find SLA IDs. Priorities are per-SLA, not global.",
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
        "list_open_tickets" => exec_list_open_tickets(args, client).await,
        "list_tickets_by_client" => exec_list_tickets_by_client(args, client).await,
        "list_high_priority_tickets" => exec_list_high_priority_tickets(args, client).await,
        "list_unassigned_tickets" => exec_list_unassigned_tickets(args, client).await,
        "list_my_tickets" => exec_list_my_tickets(args, client).await,
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
        "list_statuses" => exec_list_statuses(args, client).await,
        "get_status_details" => exec_get_status_details(args, client).await,
        "global_search" => exec_global_search(args, client).await,
        "list_field_groups" => exec_list_field_groups(client).await,
        "list_asset_changes" => exec_list_asset_changes(args, client).await,
        "create_invoice" => exec_create_invoice(args, client).await,
        "void_invoice" => exec_void_invoice(args, client).await,
        "list_suppliers" => exec_list_suppliers(args, client).await,
        "get_supplier" => exec_get_supplier(args, client).await,
        "create_supplier" => exec_create_supplier(args, client).await,
        "create_supplier_user" => exec_create_supplier_user(args, client).await,
        "list_sales_orders" => exec_list_sales_orders(args, client).await,
        "get_sales_order" => exec_get_sales_order(args, client).await,
        "create_sales_order" => exec_create_sales_order(args, client).await,
        "get_audit_entry" => exec_get_audit_entry(args, client).await,
        "get_field" => exec_get_field(args, client).await,
        "get_pax8_data" => exec_get_pax8_data(args, client).await,
        "get_meraki_data" => exec_get_meraki_data(client).await,
        "import_from_xero" => exec_import_from_xero(args, client).await,
        "list_approval_processes" => exec_list_approval_processes(client).await,
        "get_approval_process" => exec_get_approval_process(args, client).await,
        "create_approval_process" => exec_create_approval_process(args, client).await,
        "list_approval_rules" => exec_list_approval_rules(args, client).await,
        "create_approval_rule" => exec_create_approval_rule(args, client).await,
        "list_views" => exec_list_views(args, client).await,
        "get_view" => exec_get_view(args, client).await,
        "list_view_filters" => exec_list_view_filters(args, client).await,
        "list_view_columns" => exec_list_view_columns(args, client).await,
        "create_client" => exec_create_client(args, client).await,
        "list_invoice_payments" => exec_list_invoice_payments(args, client).await,
        "list_workflows" => exec_list_workflows(args, client).await,
        "get_workflow" => exec_get_workflow(args, client).await,
        "create_workflow" => exec_create_workflow(args, client).await,
        "update_workflow" => exec_update_workflow(args, client).await,
        "delete_workflow" => exec_delete_workflow(args, client).await,
        "list_notifications" => exec_list_notifications(args, client).await,
        "get_notification" => exec_get_notification(args, client).await,
        "create_notification" => exec_create_notification(args, client).await,
        "send_notification_message" => exec_send_notification_message(args, client).await,
        "list_services" => exec_list_services(args, client).await,
        "get_service" => exec_get_service(args, client).await,
        "list_service_categories" => exec_list_service_categories(client).await,
        "list_service_statuses" => exec_list_service_statuses(args, client).await,
        "create_service_status" => exec_create_service_status(args, client).await,
        "list_invoice_statuses" => exec_list_invoice_statuses(client).await,
        "get_invoice_status" => exec_get_invoice_status(args, client).await,
        "create_invoice_status" => exec_create_invoice_status(args, client).await,
        "delete_invoice_status" => exec_delete_invoice_status(args, client).await,
        "update_invoice_lines" => exec_update_invoice_lines(args, client).await,
        "update_sales_order_lines" => exec_update_sales_order_lines(args, client).await,
        "register_invoice_view" => exec_register_invoice_view(args, client).await,
        "register_sales_order_view" => exec_register_sales_order_view(args, client).await,
        "register_purchase_order_view" => exec_register_purchase_order_view(args, client).await,
        "register_kb_article_view" => exec_register_kb_article_view(args, client).await,
        "review_expense" => exec_review_expense(args, client).await,
        "expire_client_prepay" => exec_expire_client_prepay(args, client).await,
        "list_integration_configs" => exec_list_integration_configs(client).await,
        "get_integration_config" => exec_get_integration_config(args, client).await,
        "list_integration_site_mappings" => exec_list_integration_site_mappings(args, client).await,
        "list_integration_errors" => exec_list_integration_errors(args, client).await,
        "get_integration_error" => exec_get_integration_error(args, client).await,
        "list_integration_requests" => exec_list_integration_requests(args, client).await,
        "get_integration_request" => exec_get_integration_request(args, client).await,
        "list_integration_field_mappings" => exec_list_integration_field_mappings(args, client).await,
        "get_microsoft_csp_data" => exec_get_microsoft_csp_data(args, client).await,
        "get_intune_data" => exec_get_intune_data(args, client).await,
        "get_azure_ad_data" => exec_get_azure_ad_data(args, client).await,
        "get_ninja_rmm_data" => exec_get_ninja_rmm_data(args, client).await,
        "get_xero_data" => exec_get_xero_data(args, client).await,
        "list_xero_details" => exec_list_xero_details(args, client).await,
        "get_xero_detail" => exec_get_xero_detail(args, client).await,
        "send_invoice_to_xero" => exec_send_invoice_to_xero(args, client).await,
        "create_asset" => exec_create_asset(args, client).await,
        "update_asset" => exec_update_asset(args, client).await,
        "list_asset_software" => exec_list_asset_software(args, client).await,
        "list_roles" => exec_list_roles(args, client).await,
        "get_role" => exec_get_role(args, client).await,
        "create_role" => exec_create_role(args, client).await,
        "update_role" => exec_update_role(args, client).await,
        "delete_role" => exec_delete_role(args, client).await,
        "list_tags" => exec_list_tags(args, client).await,
        "get_tag" => exec_get_tag(args, client).await,
        "create_tag" => exec_create_tag(args, client).await,
        "delete_tag" => exec_delete_tag(args, client).await,
        "list_item_groups" => exec_list_item_groups(args, client).await,
        "get_item_group" => exec_get_item_group(args, client).await,
        "create_item_group" => exec_create_item_group(args, client).await,
        "delete_item_group" => exec_delete_item_group(args, client).await,
        "list_items" => exec_list_items(args, client).await,
        "get_item" => exec_get_item(args, client).await,
        "create_item" => exec_create_item(args, client).await,
        "list_item_stock" => exec_list_item_stock(args, client).await,
        "list_products" => exec_list_products(args, client).await,
        "get_product" => exec_get_product(args, client).await,
        "create_product" => exec_create_product(args, client).await,
        "delete_product" => exec_delete_product(args, client).await,
        "list_product_components" => exec_list_product_components(args, client).await,
        "create_product_component" => exec_create_product_component(args, client).await,
        "list_quotations" => exec_list_quotations(args, client).await,
        "get_quotation" => exec_get_quotation(args, client).await,
        "create_quotation" => exec_create_quotation(args, client).await,
        "update_quotation_lines" => exec_update_quotation_lines(args, client).await,
        "approve_quotation" => exec_approve_quotation(args, client).await,
        "view_quotation" => exec_view_quotation(args, client).await,
        "list_timesheets" => exec_list_timesheets(args, client).await,
        "get_my_timesheets" => exec_get_my_timesheets(client).await,
        "get_timesheet" => exec_get_timesheet(args, client).await,
        "create_timesheet" => exec_create_timesheet(args, client).await,
        "get_invoice" => exec_get_invoice(args, client).await,
        "list_invoice_lines" => exec_list_invoice_lines(args, client).await,
        "create_asset_group" => exec_create_asset_group(args, client).await,
        "update_asset_group" => exec_update_asset_group(args, client).await,
        "delete_asset_group" => exec_delete_asset_group(args, client).await,
        "list_attachments" => exec_list_attachments(args, client).await,
        "get_attachment" => exec_get_attachment(args, client).await,
        "delete_attachment" => exec_delete_attachment(args, client).await,
        "update_client" => exec_update_client(args, client).await,
        "list_asset_types" => exec_list_asset_types(client).await,
        "search_agents" => exec_search_agents(args, client).await,
        "list_projects" => exec_list_projects(args, client).await,
        "get_project" => exec_get_project(args, client).await,
        "create_project" => exec_create_project(args, client).await,
        "update_project" => exec_update_project(args, client).await,
        "list_project_tasks" => exec_list_project_tasks(args, client).await,
        "create_report_pdf" => exec_create_report_pdf(args, client).await,
        "list_opportunities" => exec_list_opportunities(args, client).await,
        "get_opportunity" => exec_get_opportunity(args, client).await,
        "create_opportunity" => exec_create_opportunity(args, client).await,
        "update_opportunity" => exec_update_opportunity(args, client).await,
        "list_crm_notes" => exec_list_crm_notes(args, client).await,
        "create_crm_note" => exec_create_crm_note(args, client).await,
        "list_contact_groups" => exec_list_contact_groups(client).await,
        "manage_contact_group_members" => exec_manage_contact_group_members(args, client).await,
        "list_ticket_approvals" => exec_list_ticket_approvals(args, client).await,
        "process_approval" => exec_process_approval(args, client).await,
        "list_feedback" => exec_list_feedback(args, client).await,
        "list_custom_tables" => exec_list_custom_tables(client).await,
        "get_custom_table" => exec_get_custom_table(args, client).await,
        "create_custom_table" => exec_create_custom_table(args, client).await,
        "delete_custom_table" => exec_delete_custom_table(args, client).await,
        "list_charge_rates" => exec_list_charge_rates(client).await,
        "list_billing_lines" => exec_list_billing_lines(args, client).await,
        "list_invoices" => exec_list_invoices(args, client).await,
        "list_recurring_invoices" => exec_list_recurring_invoices(args, client).await,
        "get_recurring_invoice" => exec_get_recurring_invoice(args, client).await,
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
        "list_ticket_areas" => exec_list_ticket_areas(client).await,
        "get_ticket_type_details" => exec_get_ticket_type_details(args, client).await,
        "list_ticket_type_fields" => exec_list_ticket_type_fields(args, client).await,
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
        "list_priorities" => exec_list_priorities(args, client).await,
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

/// HaloPSA embeds plaintext auto-generated credentials in some responses —
/// e.g. creating a client auto-provisions a default portal contact and
/// returns its password as `new_password` nested under `site_update[].
/// users_update[]`. Strip these recursively before any client/site/user/
/// supplier object reaches the model or conversation transcript.
fn redact_secrets(value: &mut Value) {
    const SENSITIVE_KEYS: &[&str] = &["new_password", "password", "passwordconfirm"];
    match value {
        Value::Object(map) => {
            for key in SENSITIVE_KEYS {
                map.remove(*key);
            }
            for v in map.values_mut() {
                redact_secrets(v);
            }
        }
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                redact_secrets(v);
            }
        }
        _ => {}
    }
}

/// A single ticket fetched/created/updated with `includedetails=true`
/// embeds the full workflow-action field schema (every field, dropdown
/// value, and validation rule for every action available on the ticket's
/// type) under `extra_actions` — observed: ~94KB / 3200 lines for one
/// freshly created ticket, almost entirely schema noise. Nothing in this
/// codebase reads `extra_actions`, so drop it before returning.
fn strip_ticket_bloat(value: &mut Value) {
    if let Some(obj) = value.as_object_mut() {
        obj.remove("extra_actions");
    }
}

fn summarize_tickets(tickets: &[Value]) -> Vec<Value> {
    tickets
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
        .collect()
}

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
        priority: args.get("priority").and_then(|v| v.as_str()).map(String::from),
        ticketarea_id: args.get("ticketarea_id").and_then(|v| v.as_i64()),
        parent_id: args.get("parent_id").and_then(|v| v.as_i64()),
    };

    let (tickets, total) = client.list_tickets(page, page_size, &filter).await?;

    Ok(serde_json::to_string_pretty(&json!({
        "tickets": summarize_tickets(&tickets),
        "total_count": total,
        "page": page,
        "page_size": page_size,
    }))
    .unwrap())
}

async fn exec_list_open_tickets(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);

    let filter = TicketFilter {
        open_only: true,
        ..Default::default()
    };
    let (tickets, total) = client.list_tickets(page, page_size, &filter).await?;

    Ok(serde_json::to_string_pretty(&json!({
        "tickets": summarize_tickets(&tickets),
        "total_count": total,
        "page": page,
        "page_size": page_size,
    }))
    .unwrap())
}

async fn exec_list_tickets_by_client(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let client_id = args
        .get("client_id")
        .and_then(|v| v.as_i64())
        .ok_or("client_id is required")?;
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);

    let filter = TicketFilter {
        client_id: Some(client_id),
        ..Default::default()
    };
    let (tickets, total) = client.list_tickets(page, page_size, &filter).await?;

    Ok(serde_json::to_string_pretty(&json!({
        "tickets": summarize_tickets(&tickets),
        "total_count": total,
        "page": page,
        "page_size": page_size,
    }))
    .unwrap())
}

async fn exec_list_high_priority_tickets(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);
    let labels: Vec<String> = args
        .get("priority_labels")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_else(|| vec!["High".into(), "Critical".into(), "RFO".into()]);

    // Only single-label filtering per call is confirmed — query each
    // label separately and merge/dedupe by id rather than guessing at
    // OR-across-values semantics within one advanced_search call.
    let mut seen = std::collections::HashSet::new();
    let mut merged: Vec<Value> = Vec::new();
    let mut total = 0i64;
    for label in &labels {
        let filter = TicketFilter {
            priority: Some(label.clone()),
            ..Default::default()
        };
        let (tickets, count) = client.list_tickets(1, page_size, &filter).await?;
        total += count;
        for t in tickets {
            if let Some(id) = t.get("id").and_then(|v| v.as_i64()) {
                if seen.insert(id) {
                    merged.push(t);
                }
            }
        }
    }
    merged.truncate(page_size.max(1) as usize);

    Ok(serde_json::to_string_pretty(&json!({
        "tickets": summarize_tickets(&merged),
        "total_count": total,
        "priority_labels": labels,
        "note": "total_count is the sum across each priority label's own count, not deduplicated; tickets array is deduplicated and truncated to page_size",
    }))
    .unwrap())
}

async fn exec_list_unassigned_tickets(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);

    // Confirmed against the agent UI: unassigned tickets are represented
    // by a reserved placeholder agent_id=1, not a null/missing value.
    let filter = TicketFilter {
        agent_id: Some(1),
        ..Default::default()
    };
    let (tickets, total) = client.list_tickets(page, page_size, &filter).await?;

    Ok(serde_json::to_string_pretty(&json!({
        "tickets": summarize_tickets(&tickets),
        "total_count": total,
        "page": page,
        "page_size": page_size,
    }))
    .unwrap())
}

async fn exec_list_my_tickets(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);

    let me = client.get_me().await?;
    let agent_id = me
        .get("agentid")
        .or_else(|| me.get("agent_id"))
        .or_else(|| me.get("id"))
        .and_then(|v| v.as_i64())
        .ok_or("Could not resolve your agent ID from get_me — fall back to list_tickets with an explicit agent_id")?;

    let filter = TicketFilter {
        agent_id: Some(agent_id),
        ..Default::default()
    };
    let (tickets, total) = client.list_tickets(page, page_size, &filter).await?;

    Ok(serde_json::to_string_pretty(&json!({
        "tickets": summarize_tickets(&tickets),
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
    strip_ticket_bloat(&mut result);
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

    let mut result = client.create_ticket(ticket).await?;
    strip_ticket_bloat(&mut result);
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

    let mut result = client.update_ticket(ticket_id, fields).await?;
    strip_ticket_bloat(&mut result);
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

async fn exec_list_statuses(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let status_type = args.get("status_type").and_then(|v| v.as_str());
    let statuses = client.list_statuses(status_type).await?;
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

async fn exec_get_status_details(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let status_id = args
        .get("status_id")
        .and_then(|v| v.as_i64())
        .ok_or("status_id is required")?;

    let result = client.get_status_details(status_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_global_search(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or("query is required")?;
    let count_per_entity = args
        .get("count_per_entity")
        .and_then(|v| v.as_i64())
        .unwrap_or(3);

    let result = client.global_search(query, count_per_entity).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_field_groups(client: &HaloPSAClient) -> Result<String, String> {
    let groups = client.list_field_groups().await?;
    Ok(serde_json::to_string_pretty(&json!({ "field_groups": groups })).unwrap())
}

async fn exec_list_asset_changes(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let asset_id = args.get("asset_id").and_then(|v| v.as_i64());
    let count = args.get("count").and_then(|v| v.as_i64()).unwrap_or(200);
    let changes = client.list_asset_changes(asset_id, count).await?;
    Ok(serde_json::to_string_pretty(&json!({ "changes": changes })).unwrap())
}

async fn exec_create_invoice(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;
    let result = client.create_invoice(fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_void_invoice(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let invoice_id = args.get("invoice_id").and_then(|v| v.as_i64()).ok_or("invoice_id is required")?;
    let result = client.void_invoice(invoice_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_suppliers(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let count = args.get("count").and_then(|v| v.as_i64()).unwrap_or(50);
    let suppliers = client.list_suppliers(count).await?;
    Ok(serde_json::to_string_pretty(&json!({ "suppliers": suppliers })).unwrap())
}

async fn exec_get_supplier(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let supplier_id = args.get("supplier_id").and_then(|v| v.as_i64()).ok_or("supplier_id is required")?;
    let mut result = client.get_supplier(supplier_id).await?;
    redact_secrets(&mut result);
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_create_supplier(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;
    let mut result = client.create_supplier(fields).await?;
    redact_secrets(&mut result);
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_create_supplier_user(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let supplier_id = args.get("supplier_id").and_then(|v| v.as_i64()).ok_or("supplier_id is required")?;
    let supplier_name = args.get("supplier_name").and_then(|v| v.as_str()).ok_or("supplier_name is required")?;
    let fields = args.get("fields").cloned().ok_or("fields is required")?;
    let mut result = client.create_supplier_user(supplier_id, supplier_name, fields).await?;
    redact_secrets(&mut result);
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_sales_orders(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let client_id = args.get("client_id").and_then(|v| v.as_i64());
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);
    let orders = client.list_sales_orders(client_id, page, page_size).await?;
    Ok(serde_json::to_string_pretty(&json!({ "sales_orders": orders })).unwrap())
}

async fn exec_get_sales_order(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let sales_order_id = args.get("sales_order_id").and_then(|v| v.as_i64()).ok_or("sales_order_id is required")?;
    let result = client.get_sales_order(sales_order_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_create_sales_order(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;
    let result = client.create_sales_order(fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_get_audit_entry(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let audit_entry_id = args.get("audit_entry_id").and_then(|v| v.as_i64()).ok_or("audit_entry_id is required")?;
    let result = client.get_audit_entry(audit_entry_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_get_field(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let field_id = args.get("field_id").and_then(|v| v.as_i64()).ok_or("field_id is required")?;
    let result = client.get_field(field_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_get_pax8_data(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let datatype = args.get("datatype").and_then(|v| v.as_str()).ok_or("datatype is required")?;
    let search = args.get("search").and_then(|v| v.as_str());
    let result = client.get_pax8_data(Some(datatype), search).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_get_meraki_data(client: &HaloPSAClient) -> Result<String, String> {
    let result = client.get_meraki_data().await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_import_from_xero(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let options = args.get("options").cloned().unwrap_or(json!({}));
    let result = client.import_from_xero(options).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_approval_processes(client: &HaloPSAClient) -> Result<String, String> {
    let processes = client.list_approval_processes().await?;
    Ok(serde_json::to_string_pretty(&json!({ "approval_processes": processes })).unwrap())
}

async fn exec_get_approval_process(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let process_id = args.get("process_id").and_then(|v| v.as_i64()).ok_or("process_id is required")?;
    let result = client.get_approval_process(process_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_create_approval_process(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;
    let result = client.create_approval_process(fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_approval_rules(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let process_id = args.get("process_id").and_then(|v| v.as_i64());
    let rules = client.list_approval_rules(process_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "approval_rules": rules })).unwrap())
}

async fn exec_create_approval_rule(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;
    let result = client.create_approval_rule(fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_views(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let domain = args.get("domain").and_then(|v| v.as_str()).ok_or("domain is required")?;
    let view_type = args.get("view_type").and_then(|v| v.as_str()).ok_or("view_type is required")?;
    let views = client.list_views(domain, view_type).await?;
    Ok(serde_json::to_string_pretty(&json!({ "views": views })).unwrap())
}

async fn exec_get_view(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let view_id = args.get("view_id").and_then(|v| v.as_i64()).ok_or("view_id is required")?;
    let result = client.get_view(view_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_view_filters(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let view_type = args.get("view_type").and_then(|v| v.as_str()).ok_or("view_type is required")?;
    let ticketarea_id = args.get("ticketarea_id").and_then(|v| v.as_i64());
    let filters = client.list_view_filters(view_type, ticketarea_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "filters": filters })).unwrap())
}

async fn exec_list_view_columns(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let view_type = args.get("view_type").and_then(|v| v.as_str()).ok_or("view_type is required")?;
    let ticketarea_id = args.get("ticketarea_id").and_then(|v| v.as_i64());
    let columns = client.list_view_columns(view_type, ticketarea_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "columns": columns })).unwrap())
}

async fn exec_create_client(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;
    let mut result = client.create_client(fields).await?;
    redact_secrets(&mut result);
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_invoice_payments(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let invoice_id = args.get("invoice_id").and_then(|v| v.as_i64());
    let count = args.get("count").and_then(|v| v.as_i64()).unwrap_or(50);
    let payments = client.list_invoice_payments(invoice_id, count).await?;
    Ok(serde_json::to_string_pretty(&json!({ "payments": payments })).unwrap())
}

async fn exec_list_workflows(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);
    let workflows = client.list_workflows(page, page_size).await?;
    Ok(serde_json::to_string_pretty(&json!({ "workflows": workflows })).unwrap())
}

async fn exec_get_workflow(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let workflow_id = args.get("workflow_id").and_then(|v| v.as_i64()).ok_or("workflow_id is required")?;
    let result = client.get_workflow_details(workflow_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_create_workflow(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;
    let result = client.create_workflow(fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_update_workflow(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let workflow_id = args.get("workflow_id").and_then(|v| v.as_i64()).ok_or("workflow_id is required")?;
    let fields = args.get("fields").cloned().unwrap_or(json!({}));
    let result = client.update_workflow(workflow_id, fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_delete_workflow(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let workflow_id = args.get("workflow_id").and_then(|v| v.as_i64()).ok_or("workflow_id is required")?;
    client.delete_workflow(workflow_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "deleted": true, "workflow_id": workflow_id })).unwrap())
}

async fn exec_list_notifications(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let agent_id = args.get("agent_id").and_then(|v| v.as_i64());
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);
    let notifications = client.list_notifications(agent_id, page, page_size).await?;
    Ok(serde_json::to_string_pretty(&json!({ "notifications": notifications })).unwrap())
}

async fn exec_get_notification(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let notification_id = args.get("notification_id").and_then(|v| v.as_i64()).ok_or("notification_id is required")?;
    let result = client.get_notification(notification_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_create_notification(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;
    let result = client.create_notification(fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_send_notification_message(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;
    let result = client.send_notification_message(fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_services(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let search = args.get("search").and_then(|v| v.as_str());
    let category_id = args.get("category_id").and_then(|v| v.as_i64());
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);
    let (services, total) = client.list_services(search, category_id, page, page_size).await?;
    Ok(serde_json::to_string_pretty(&json!({ "services": services, "total_count": total, "page": page, "page_size": page_size })).unwrap())
}

async fn exec_get_service(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let service_id = args.get("service_id").and_then(|v| v.as_i64()).ok_or("service_id is required")?;
    let result = client.get_service(service_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_service_categories(client: &HaloPSAClient) -> Result<String, String> {
    let categories = client.list_service_categories().await?;
    Ok(serde_json::to_string_pretty(&json!({ "service_categories": categories })).unwrap())
}

async fn exec_list_service_statuses(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let service_id = args.get("service_id").and_then(|v| v.as_i64());
    let statuses = client.list_service_statuses(service_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "statuses": statuses })).unwrap())
}

async fn exec_create_service_status(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;
    let result = client.create_service_status(fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_invoice_statuses(client: &HaloPSAClient) -> Result<String, String> {
    let statuses = client.list_invoice_statuses().await?;
    Ok(serde_json::to_string_pretty(&json!({ "invoice_statuses": statuses })).unwrap())
}

async fn exec_get_invoice_status(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let status_id = args.get("status_id").and_then(|v| v.as_i64()).ok_or("status_id is required")?;
    let result = client.get_invoice_status(status_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_create_invoice_status(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;
    let result = client.create_invoice_status(fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_delete_invoice_status(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let status_id = args.get("status_id").and_then(|v| v.as_i64()).ok_or("status_id is required")?;
    client.delete_invoice_status(status_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "deleted": true, "status_id": status_id })).unwrap())
}

async fn exec_update_invoice_lines(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let lines = args.get("lines").cloned().ok_or("lines is required")?;
    let result = client.update_invoice_lines(lines).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_update_sales_order_lines(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let lines = args.get("lines").cloned().ok_or("lines is required")?;
    let result = client.update_sales_order_lines(lines).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_register_invoice_view(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let invoice_id = args.get("invoice_id").and_then(|v| v.as_i64()).ok_or("invoice_id is required")?;
    let result = client.register_invoice_view(invoice_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_register_sales_order_view(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let sales_order_id = args.get("sales_order_id").and_then(|v| v.as_i64()).ok_or("sales_order_id is required")?;
    let result = client.register_sales_order_view(sales_order_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_register_purchase_order_view(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let purchase_order_id = args.get("purchase_order_id").and_then(|v| v.as_i64()).ok_or("purchase_order_id is required")?;
    let result = client.register_purchase_order_view(purchase_order_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_register_kb_article_view(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let kb_article_id = args.get("kb_article_id").and_then(|v| v.as_i64()).ok_or("kb_article_id is required")?;
    let result = client.register_kb_article_view(kb_article_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_review_expense(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let expense_ids: Vec<i64> = args
        .get("expense_ids")
        .and_then(|v| v.as_array())
        .ok_or("expense_ids is required")?
        .iter()
        .filter_map(|v| v.as_i64())
        .collect();
    let result = client.review_expense(&expense_ids).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_expire_client_prepay(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let prepay_ids: Vec<i64> = args
        .get("prepay_ids")
        .and_then(|v| v.as_array())
        .ok_or("prepay_ids is required")?
        .iter()
        .filter_map(|v| v.as_i64())
        .collect();
    let result = client.expire_client_prepay(&prepay_ids).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_integration_configs(client: &HaloPSAClient) -> Result<String, String> {
    let configs = client.list_integration_configs().await?;
    Ok(serde_json::to_string_pretty(&json!({ "integration_configs": configs })).unwrap())
}

async fn exec_get_integration_config(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let config_id = args.get("config_id").and_then(|v| v.as_i64()).ok_or("config_id is required")?;
    let result = client.get_integration_config(config_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_integration_site_mappings(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let module_id = args.get("module_id").and_then(|v| v.as_i64());
    let mappings = client.list_integration_site_mappings(module_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "site_mappings": mappings })).unwrap())
}

async fn exec_list_integration_errors(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let module_id = args.get("module_id").and_then(|v| v.as_i64());
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);
    let errors = client.list_integration_errors(module_id, page, page_size).await?;
    Ok(serde_json::to_string_pretty(&json!({ "errors": errors })).unwrap())
}

async fn exec_get_integration_error(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let error_id = args.get("error_id").and_then(|v| v.as_i64()).ok_or("error_id is required")?;
    let result = client.get_integration_error(error_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_integration_requests(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let module_id = args.get("module_id").and_then(|v| v.as_i64());
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);
    let requests = client.list_integration_requests(module_id, page, page_size).await?;
    Ok(serde_json::to_string_pretty(&json!({ "requests": requests })).unwrap())
}

async fn exec_get_integration_request(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let request_id = args.get("request_id").and_then(|v| v.as_i64()).ok_or("request_id is required")?;
    let result = client.get_integration_request(request_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_integration_field_mappings(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let module_id = args.get("module_id").and_then(|v| v.as_i64());
    let mappings = client.list_integration_field_mappings(module_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "field_mappings": mappings })).unwrap())
}

async fn exec_get_microsoft_csp_data(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let datatype = args.get("datatype").and_then(|v| v.as_str()).ok_or("datatype is required")?;
    let search = args.get("search").and_then(|v| v.as_str());
    let result = client.get_microsoft_csp_data(Some(datatype), search).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_get_intune_data(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let datatype = args.get("datatype").and_then(|v| v.as_str()).ok_or("datatype is required")?;
    let search = args.get("search").and_then(|v| v.as_str());
    let result = client.get_intune_data(Some(datatype), search).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_get_azure_ad_data(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let datatype = args.get("datatype").and_then(|v| v.as_str()).ok_or("datatype is required")?;
    let search = args.get("search").and_then(|v| v.as_str());
    let result = client.get_azure_ad_data(Some(datatype), search).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_get_ninja_rmm_data(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let datatype = args.get("datatype").and_then(|v| v.as_str()).ok_or("datatype is required")?;
    let search = args.get("search").and_then(|v| v.as_str());
    let result = client.get_ninja_rmm_data(Some(datatype), search).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_get_xero_data(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let datatype = args.get("datatype").and_then(|v| v.as_str()).ok_or("datatype is required")?;
    let search = args.get("search").and_then(|v| v.as_str());
    let result = client.get_xero_data(Some(datatype), search).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_xero_details(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);
    let details = client.list_xero_details(page, page_size).await?;
    Ok(serde_json::to_string_pretty(&json!({ "xero_details": details })).unwrap())
}

async fn exec_get_xero_detail(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let detail_id = args.get("detail_id").and_then(|v| v.as_i64()).ok_or("detail_id is required")?;
    let result = client.get_xero_detail(detail_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_send_invoice_to_xero(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let invoice_id = args.get("invoice_id").and_then(|v| v.as_i64()).ok_or("invoice_id is required")?;
    let result = client.send_invoice_to_xero(invoice_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_create_asset(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;
    let result = client.create_asset(fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_update_asset(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let asset_id = args.get("asset_id").and_then(|v| v.as_i64()).ok_or("asset_id is required")?;
    let fields = args.get("fields").cloned().unwrap_or(json!({}));
    let mut result = client.update_asset(asset_id, fields).await?;
    if let Some(obj) = result.as_object_mut() {
        obj.remove("fields");
    }
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_asset_software(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let asset_id = args.get("asset_id").and_then(|v| v.as_i64()).ok_or("asset_id is required")?;
    let software = client.list_asset_software(asset_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "software": software })).unwrap())
}

async fn exec_list_roles(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);

    let (roles, total) = client.list_roles(page, page_size).await?;
    Ok(serde_json::to_string_pretty(&json!({ "roles": roles, "total_count": total, "page": page, "page_size": page_size })).unwrap())
}

async fn exec_get_role(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let role_id = args.get("role_id").and_then(|v| v.as_i64()).ok_or("role_id is required")?;
    let result = client.get_role(role_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_create_role(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;
    let result = client.create_role(fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_update_role(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let role_id = args.get("role_id").and_then(|v| v.as_i64()).ok_or("role_id is required")?;
    let fields = args.get("fields").cloned().unwrap_or(json!({}));
    let result = client.update_role(role_id, fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_delete_role(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let role_id = args.get("role_id").and_then(|v| v.as_i64()).ok_or("role_id is required")?;
    client.delete_role(role_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "deleted": true, "role_id": role_id })).unwrap())
}

async fn exec_list_tags(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let search = args.get("search").and_then(|v| v.as_str());
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);

    let tags = client.list_tags(search, page, page_size).await?;
    Ok(serde_json::to_string_pretty(&json!({ "tags": tags })).unwrap())
}

async fn exec_get_tag(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let tag_id = args.get("tag_id").and_then(|v| v.as_i64()).ok_or("tag_id is required")?;
    let result = client.get_tag(tag_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_create_tag(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;
    let result = client.create_tag(fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_delete_tag(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let tag_id = args.get("tag_id").and_then(|v| v.as_i64()).ok_or("tag_id is required")?;
    client.delete_tag(tag_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "deleted": true, "tag_id": tag_id })).unwrap())
}

async fn exec_list_item_groups(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);
    let groups = client.list_item_groups(page, page_size).await?;
    Ok(serde_json::to_string_pretty(&json!({ "item_groups": groups })).unwrap())
}

async fn exec_get_item_group(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let group_id = args.get("group_id").and_then(|v| v.as_i64()).ok_or("group_id is required")?;
    let result = client.get_item_group(group_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_create_item_group(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;
    let result = client.create_item_group(fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_delete_item_group(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let group_id = args.get("group_id").and_then(|v| v.as_i64()).ok_or("group_id is required")?;
    client.delete_item_group(group_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "deleted": true, "group_id": group_id })).unwrap())
}

async fn exec_list_items(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let search = args.get("search").and_then(|v| v.as_str());
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);

    let (items, total) = client.list_items(search, page, page_size).await?;
    Ok(serde_json::to_string_pretty(&json!({ "items": items, "total_count": total, "page": page, "page_size": page_size })).unwrap())
}

async fn exec_get_item(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let item_id = args.get("item_id").and_then(|v| v.as_i64()).ok_or("item_id is required")?;
    let result = client.get_item(item_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_create_item(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;
    let result = client.create_item(fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_item_stock(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let item_id = args.get("item_id").and_then(|v| v.as_i64());
    let stock = client.list_item_stock(item_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "item_stock": stock })).unwrap())
}

async fn exec_list_products(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let search = args.get("search").and_then(|v| v.as_str());
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);
    let products = client.list_products(search, page, page_size).await?;
    Ok(serde_json::to_string_pretty(&json!({ "products": products })).unwrap())
}

async fn exec_get_product(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let product_id = args.get("product_id").and_then(|v| v.as_i64()).ok_or("product_id is required")?;
    let result = client.get_product(product_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_create_product(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;
    let result = client.create_product(fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_delete_product(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let product_id = args.get("product_id").and_then(|v| v.as_i64()).ok_or("product_id is required")?;
    client.delete_product(product_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "deleted": true, "product_id": product_id })).unwrap())
}

async fn exec_list_product_components(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let product_id = args.get("product_id").and_then(|v| v.as_i64());
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);
    let components = client.list_product_components(product_id, page, page_size).await?;
    Ok(serde_json::to_string_pretty(&json!({ "components": components })).unwrap())
}

async fn exec_create_product_component(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;
    let result = client.create_product_component(fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_quotations(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let client_id = args.get("client_id").and_then(|v| v.as_i64());
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);

    let (quotations, total) = client.list_quotations(client_id, page, page_size).await?;
    Ok(serde_json::to_string_pretty(&json!({ "quotations": quotations, "total_count": total, "page": page, "page_size": page_size })).unwrap())
}

async fn exec_get_quotation(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let quotation_id = args.get("quotation_id").and_then(|v| v.as_i64()).ok_or("quotation_id is required")?;
    let result = client.get_quotation(quotation_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_create_quotation(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;
    let result = client.create_quotation(fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_update_quotation_lines(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let quotation_id = args.get("quotation_id").and_then(|v| v.as_i64()).ok_or("quotation_id is required")?;
    let lines = args.get("lines").cloned().ok_or("lines is required")?;
    let result = client.update_quotation_lines(quotation_id, lines).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_approve_quotation(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let quotation_id = args.get("quotation_id").and_then(|v| v.as_i64()).ok_or("quotation_id is required")?;
    let approved = args.get("approved").and_then(|v| v.as_bool()).ok_or("approved is required")?;
    let notes = args.get("notes").and_then(|v| v.as_str());
    let result = client.approve_quotation(quotation_id, approved, notes).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_view_quotation(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let quotation_id = args.get("quotation_id").and_then(|v| v.as_i64()).ok_or("quotation_id is required")?;
    let result = client.view_quotation(quotation_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_timesheets(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let agent_id = args.get("agent_id").and_then(|v| v.as_i64());
    let entries = client.list_timesheets(agent_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "timesheets": entries })).unwrap())
}

async fn exec_get_my_timesheets(client: &HaloPSAClient) -> Result<String, String> {
    let entries = client.get_my_timesheets().await?;
    Ok(serde_json::to_string_pretty(&json!({ "timesheets": entries })).unwrap())
}

async fn exec_get_timesheet(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let timesheet_id = args.get("timesheet_id").and_then(|v| v.as_i64()).ok_or("timesheet_id is required")?;
    let result = client.get_timesheet(timesheet_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_create_timesheet(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;
    let result = client.create_timesheet(fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_get_invoice(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let invoice_id = args.get("invoice_id").and_then(|v| v.as_i64()).ok_or("invoice_id is required")?;
    let result = client.get_invoice(invoice_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_invoice_lines(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let invoice_id = args.get("invoice_id").and_then(|v| v.as_i64());
    let lines = client.list_invoice_lines(invoice_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "lines": lines })).unwrap())
}

async fn exec_create_asset_group(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;
    let result = client.create_asset_group(fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_update_asset_group(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let group_id = args.get("group_id").and_then(|v| v.as_i64()).ok_or("group_id is required")?;
    let fields = args.get("fields").cloned().unwrap_or(json!({}));
    let result = client.update_asset_group(group_id, fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_delete_asset_group(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let group_id = args.get("group_id").and_then(|v| v.as_i64()).ok_or("group_id is required")?;
    client.delete_asset_group(group_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "deleted": true, "group_id": group_id })).unwrap())
}

async fn exec_list_attachments(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let ticket_id = args.get("ticket_id").and_then(|v| v.as_i64());
    let attachments = client.list_attachments(ticket_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "attachments": attachments })).unwrap())
}

async fn exec_get_attachment(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let attachment_id = args.get("attachment_id").and_then(|v| v.as_i64()).ok_or("attachment_id is required")?;
    let result = client.get_attachment(attachment_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_delete_attachment(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let attachment_id = args.get("attachment_id").and_then(|v| v.as_i64()).ok_or("attachment_id is required")?;
    client.delete_attachment(attachment_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "deleted": true, "attachment_id": attachment_id })).unwrap())
}

async fn exec_update_client(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let client_id = args.get("client_id").and_then(|v| v.as_i64()).ok_or("client_id is required")?;
    let fields = args.get("fields").cloned().unwrap_or(json!({}));

    let mut result = client.update_client(client_id, fields).await?;
    redact_secrets(&mut result);
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_asset_types(client: &HaloPSAClient) -> Result<String, String> {
    let mut types = client.list_asset_types().await?;
    // Each asset type embeds its full custom-field schema (every field
    // definition for every asset of that type) under `fields` — observed
    // ~119KB/4300 lines for ~26 types. Nothing reads it; only id/name/
    // assetgroup metadata is needed to pick an assettype_id for create_asset.
    for t in types.iter_mut() {
        if let Some(obj) = t.as_object_mut() {
            obj.remove("fields");
        }
    }
    Ok(serde_json::to_string_pretty(&json!({ "asset_types": types })).unwrap())
}

async fn exec_search_agents(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let query = args.get("query").and_then(|v| v.as_str()).ok_or("query is required")?;
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);

    let agents = client.search_agents(query, page_size).await?;
    Ok(serde_json::to_string_pretty(&json!({ "agents": agents })).unwrap())
}

async fn exec_list_projects(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);
    let client_id = args.get("client_id").and_then(|v| v.as_i64());

    let (projects, total) = client.list_projects(page, page_size, client_id).await?;

    Ok(serde_json::to_string_pretty(&json!({
        "projects": projects,
        "total_count": total,
        "page": page,
        "page_size": page_size,
    }))
    .unwrap())
}

async fn exec_get_project(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let project_id = args.get("project_id").and_then(|v| v.as_i64()).ok_or("project_id is required")?;

    let mut result = client.get_project(project_id).await?;
    strip_ticket_bloat(&mut result);
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_create_project(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;

    let mut result = client.create_project(fields).await?;
    strip_ticket_bloat(&mut result);
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_update_project(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let project_id = args.get("project_id").and_then(|v| v.as_i64()).ok_or("project_id is required")?;
    let fields = args.get("fields").cloned().unwrap_or(json!({}));

    let mut result = client.update_project(project_id, fields).await?;
    strip_ticket_bloat(&mut result);
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_project_tasks(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let project_id = args.get("project_id").and_then(|v| v.as_i64()).ok_or("project_id is required")?;
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);

    let (tasks, total) = client.list_project_tasks(project_id, page, page_size).await?;

    Ok(serde_json::to_string_pretty(&json!({
        "tasks": tasks,
        "total_count": total,
        "page": page,
        "page_size": page_size,
    }))
    .unwrap())
}

async fn exec_create_report_pdf(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let report_id = args.get("report_id").and_then(|v| v.as_i64()).ok_or("report_id is required")?;
    let filters: Vec<Value> = args
        .get("filters")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let result = client.create_report_pdf(report_id, &filters).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_opportunities(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);
    let client_id = args.get("client_id").and_then(|v| v.as_i64());

    let (opportunities, total) = client.list_opportunities(page, page_size, client_id).await?;

    Ok(serde_json::to_string_pretty(&json!({
        "opportunities": opportunities,
        "total_count": total,
        "page": page,
        "page_size": page_size,
    }))
    .unwrap())
}

async fn exec_get_opportunity(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let opportunity_id = args
        .get("opportunity_id")
        .and_then(|v| v.as_i64())
        .ok_or("opportunity_id is required")?;

    let mut result = client.get_opportunity(opportunity_id).await?;
    strip_ticket_bloat(&mut result);
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_create_opportunity(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;

    let mut result = client.create_opportunity(fields).await?;
    strip_ticket_bloat(&mut result);
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_update_opportunity(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let opportunity_id = args
        .get("opportunity_id")
        .and_then(|v| v.as_i64())
        .ok_or("opportunity_id is required")?;
    let fields = args.get("fields").cloned().unwrap_or(json!({}));

    let mut result = client.update_opportunity(opportunity_id, fields).await?;
    strip_ticket_bloat(&mut result);
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_crm_notes(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let client_id = args.get("client_id").and_then(|v| v.as_i64());
    let supplier_id = args.get("supplier_id").and_then(|v| v.as_i64());

    let notes = client.list_crm_notes(client_id, supplier_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "notes": notes })).unwrap())
}

async fn exec_create_crm_note(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;

    let result = client.create_crm_note(fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_contact_groups(client: &HaloPSAClient) -> Result<String, String> {
    let groups = client.list_contact_groups().await?;
    Ok(serde_json::to_string_pretty(&json!({ "contact_groups": groups })).unwrap())
}

async fn exec_manage_contact_group_members(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let group_id = args.get("group_id").and_then(|v| v.as_i64()).ok_or("group_id is required")?;
    let user_id = args.get("user_id").and_then(|v| v.as_i64()).ok_or("user_id is required")?;
    let add = args.get("add").and_then(|v| v.as_bool()).unwrap_or(true);

    let result = client.manage_contact_group_members(group_id, user_id, add).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_ticket_approvals(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let mine = args.get("mine").and_then(|v| v.as_bool()).unwrap_or(true);

    let approvals = client.list_ticket_approvals(mine).await?;
    Ok(serde_json::to_string_pretty(&json!({ "approvals": approvals })).unwrap())
}

async fn exec_process_approval(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let approval_ids: Vec<i64> = args
        .get("approval_ids")
        .and_then(|v| v.as_array())
        .ok_or("approval_ids is required")?
        .iter()
        .filter_map(|v| v.as_i64())
        .collect();
    let approve = args.get("approve").and_then(|v| v.as_bool()).ok_or("approve is required")?;

    let result = client.process_approval(&approval_ids, approve).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_feedback(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let client_id = args.get("client_id").and_then(|v| v.as_i64());
    let agent_id = args.get("agent_id").and_then(|v| v.as_i64());

    let feedback = client.list_feedback(client_id, agent_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "feedback": feedback })).unwrap())
}

async fn exec_list_custom_tables(client: &HaloPSAClient) -> Result<String, String> {
    let tables = client.list_custom_tables().await?;
    Ok(serde_json::to_string_pretty(&json!({ "custom_tables": tables })).unwrap())
}

async fn exec_get_custom_table(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let table_id = args.get("table_id").and_then(|v| v.as_i64()).ok_or("table_id is required")?;

    let result = client.get_custom_table(table_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_create_custom_table(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let fields = args.get("fields").cloned().ok_or("fields is required")?;

    let result = client.create_custom_table(fields).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_delete_custom_table(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let table_id = args.get("table_id").and_then(|v| v.as_i64()).ok_or("table_id is required")?;

    client.delete_custom_table(table_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "deleted": true, "table_id": table_id })).unwrap())
}

async fn exec_list_charge_rates(client: &HaloPSAClient) -> Result<String, String> {
    let rates = client.list_charge_rates().await?;
    Ok(serde_json::to_string_pretty(&json!({ "charge_rates": rates })).unwrap())
}

async fn exec_list_billing_lines(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);

    let (lines, total) = client.list_billing_lines(page, page_size).await?;

    Ok(serde_json::to_string_pretty(&json!({
        "billing_lines": lines,
        "total_count": total,
        "page": page,
        "page_size": page_size,
    }))
    .unwrap())
}

async fn exec_list_invoices(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);

    let (invoices, total) = client.list_invoices(page, page_size).await?;

    Ok(serde_json::to_string_pretty(&json!({
        "invoices": invoices,
        "total_count": total,
        "page": page,
        "page_size": page_size,
    }))
    .unwrap())
}

async fn exec_list_recurring_invoices(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);

    let (invoices, total) = client.list_recurring_invoices(page, page_size).await?;

    Ok(serde_json::to_string_pretty(&json!({
        "recurring_invoices": invoices,
        "total_count": total,
        "page": page,
        "page_size": page_size,
    }))
    .unwrap())
}

async fn exec_get_recurring_invoice(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let recurring_invoice_id = args
        .get("recurring_invoice_id")
        .and_then(|v| v.as_i64())
        .ok_or("recurring_invoice_id is required")?;

    let result = client.get_recurring_invoice(recurring_invoice_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
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

    let mut result = client.get_client(client_id).await?;
    redact_secrets(&mut result);
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_clients(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1);
    let page_size = args.get("page_size").and_then(|v| v.as_i64()).unwrap_or(50);
    let search = args.get("search").and_then(|v| v.as_str());

    let (mut clients, total) = client.list_clients(page, page_size, search).await?;
    for c in clients.iter_mut() {
        redact_secrets(c);
    }

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

    let (mut clients, total) = client.search_clients(query, page_size).await?;
    for c in clients.iter_mut() {
        redact_secrets(c);
    }

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

    let (mut users, total) = client.list_users(page, page_size, client_id, search).await?;
    for u in users.iter_mut() {
        redact_secrets(u);
    }

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

    let mut result = client.get_user(user_id).await?;
    redact_secrets(&mut result);
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

    let (mut users, total) = client.search_users(query, page_size).await?;
    for u in users.iter_mut() {
        redact_secrets(u);
    }

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

    let mut result = client.get_agent(agent_id).await?;
    // Same bloat as get_me for admin agents — strip before returning.
    if let Some(obj) = result.as_object_mut() {
        obj.remove("access_control");
        obj.remove("claims");
    }
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_ticket_areas(client: &HaloPSAClient) -> Result<String, String> {
    let areas = client.list_ticket_areas().await?;
    Ok(serde_json::to_string_pretty(&json!({ "areas": areas })).unwrap())
}

async fn exec_get_ticket_type_details(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let tickettype_id = args
        .get("tickettype_id")
        .and_then(|v| v.as_i64())
        .ok_or("tickettype_id is required")?;

    let result = client.get_ticket_type(tickettype_id).await?;
    Ok(serde_json::to_string_pretty(&result).unwrap())
}

async fn exec_list_ticket_type_fields(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let tickettype_id = args
        .get("tickettype_id")
        .and_then(|v| v.as_i64())
        .ok_or("tickettype_id is required")?;

    let fields = client.list_ticket_type_fields(tickettype_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "fields": fields })).unwrap())
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
    let mut result = client.get_me().await?;
    // Full admin agents carry thousands of ACL rows and permission claims
    // here (observed: 2.8MB / ~97k lines for a global-admin sandbox agent),
    // which blows past MCP client output limits. Neither field is used by
    // identity-resolution callers (only id/agentid/name/email are), so drop
    // them from the tool-facing response.
    if let Some(obj) = result.as_object_mut() {
        obj.remove("access_control");
        obj.remove("claims");
    }
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

    let mut result = client.get_asset(asset_id).await?;
    // Same field-schema bloat pattern as list_asset_types/tickets — a
    // single asset embeds its full type's field-schema under `fields`.
    if let Some(obj) = result.as_object_mut() {
        obj.remove("fields");
    }
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

    let mut result = client.create_site(site).await?;
    redact_secrets(&mut result);
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

    let mut result = client.update_site(site_id, fields).await?;
    redact_secrets(&mut result);
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

async fn exec_list_priorities(args: &Value, client: &HaloPSAClient) -> Result<String, String> {
    let sla_id = args
        .get("sla_id")
        .and_then(|v| v.as_i64())
        .ok_or("sla_id is required")?;

    let priorities = client.list_priorities(sla_id).await?;
    Ok(serde_json::to_string_pretty(&json!({ "priorities": priorities })).unwrap())
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
