# halopsa-mcp

A Rust-based [Model Context Protocol](https://modelcontextprotocol.io/) server for [HaloPSA](https://halopsa.com/). Gives AI assistants authenticated access to HaloPSA tickets, actions, and workflows through a standards-compliant MCP interface.

The server relays OAuth 2.1 to HaloPSA's own authorization endpoint, meaning users authenticate directly with HaloPSA and all API calls carry their real permissions. The server never sees passwords or handles credentials directly.

## Features

- **OAuth 2.1 relay** with full PKCE (S256) support on both legs
- **18 MCP tools** covering tickets, actions, workflows, reference data, and semantic search
- **Workflow-aware actions** — surfaces only the transitions valid at the ticket's current step
- **Semantic search** with permission filtering per authenticated user
- **Encrypted token storage** — HaloPSA tokens stored with AES-256-GCM in SQLite
- **Hosted and on-premise** HaloPSA instances supported
- **Two MCP transports**: SSE (classic) and Streamable HTTP (POST)
- **Pluggable embedding providers**: local fastembed, OpenAI-compatible API, or Ollama

## Architecture

Cargo workspace with four crates:

| Crate | Type | Purpose |
|-------|------|---------|
| `hmcp-common` | library | Shared types, HaloPSA API client, DB traits, ticket chunking |
| `hmcp-db-sqlite` | library | SQLite backend for auth tokens and semantic embeddings |
| `hmcp-server` | binary | MCP server: OAuth 2.1 relay, SSE + Streamable HTTP transport, tool handlers |
| `hmcp-embedder` | binary | Embedding sidecar: fastembed/OpenAI/Ollama, job queue worker |

The embedder runs as a separate service. It indexes tickets using a service account (Client Credentials flow) so the embedding corpus is complete. At query time, each result is verified against the requesting user's token before being returned, so users only see tickets they have access to.

## Quick Start (Docker)

Copy `.env.example` to `.env` and fill in the required values (see [Configuration](#configuration) below), then:

```bash
docker compose up -d
```

The server starts on port `8080`. The embedder starts alongside it and begins indexing tickets in the background.

To use SSE transport with Claude Desktop or another MCP client, point it at:

```
http://your-host:8080/sse
```

For Streamable HTTP transport:

```
http://your-host:8080/mcp
```

## Configuration

Copy `.env.example` to `.env`. Required variables:

| Variable | Description |
|----------|-------------|
| `HMCP_HALO_URL` | HaloPSA instance URL (e.g. `https://psa.example.com`) |
| `HMCP_HALO_CLIENT_ID` | OAuth Application client ID (Authorization Code flow) |
| `HMCP_HALO_CLIENT_SECRET` | OAuth Application client secret |
| `HMCP_HALO_TENANT` | Tenant name for hosted HaloPSA; leave empty for on-premise |
| `HMCP_ENCRYPTION_KEY` | 32+ character key for AES-256-GCM token encryption |
| `HMCP_PUBLIC_DOMAIN` | Public domain used for OAuth redirect URIs and link generation |

Server defaults:

| Variable | Default | Description |
|----------|---------|-------------|
| `HMCP_HOST` | `0.0.0.0` | Bind address |
| `HMCP_PORT` | `8080` | Listen port |
| `HMCP_DB_BACKEND` | `sqlite` | Storage backend (`sqlite` or `postgres`) |
| `HMCP_DB_PATH` | `/data/halopsa-mcp.db` | SQLite database path |

Semantic search (optional):

| Variable | Default | Description |
|----------|---------|-------------|
| `HMCP_SEMANTIC_SEARCH` | `false` | Enable semantic search tools |
| `HMCP_EMBEDDER_URL` | `http://hmcp-embedder:8081` | Embedder sidecar URL |
| `HMCP_EMBED_CLIENT_ID` | | Service account client ID for indexing |
| `HMCP_EMBED_CLIENT_SECRET` | | Service account client secret |
| `HMCP_EMBED_TENANT` | | Service account tenant (hosted only) |
| `HMCP_WEBHOOK_SECRET` | | HaloPSA webhook secret for event notifications |
| `HMCP_EMBED_PROVIDER` | `local` | Embedding provider: `local`, `openai`, or `ollama` |
| `HMCP_EMBED_MODEL` | `BAAI/bge-base-en-v1.5` | Model name (provider-specific) |

For OpenAI-compatible providers, also set `HMCP_EMBED_API_KEY` and optionally `HMCP_EMBED_API_URL`. For Ollama, set `HMCP_EMBED_OLLAMA_URL`.

See `.env.example` for all options with comments.

## MCP Tools

### Tickets

| Tool | Description |
|------|-------------|
| `list_tickets` | List tickets with optional filters (agent, team, client, status, keyword, open-only) |
| `get_ticket` | Get full ticket details including available workflow actions |
| `create_ticket` | Create a new ticket |
| `update_ticket` | Update fields on an existing ticket |
| `search_tickets` | Keyword search across ticket summaries and details |

### Actions

| Tool | Description |
|------|-------------|
| `list_actions` | List all actions (notes, replies, transitions) on a ticket |
| `create_action` | Add a note or reply, with optional workflow transition |

### Workflow

| Tool | Description |
|------|-------------|
| `get_available_actions` | Get workflow transitions valid at the ticket's current step |
| `execute_workflow_action` | Execute a workflow transition, moving the ticket to the next step |

### Reference Data

| Tool | Description |
|------|-------------|
| `list_statuses` | List all ticket statuses with IDs |
| `list_teams` | List all teams and queues |
| `list_ticket_types` | List all ticket types |
| `get_client` | Get client details by ID |

### User

| Tool | Description |
|------|-------------|
| `get_me` | Get information about the authenticated agent |
| `get_ticket_assets` | List assets associated with a ticket |

### Semantic Search (optional, requires `HMCP_SEMANTIC_SEARCH=true`)

| Tool | Description |
|------|-------------|
| `semantic_search` | Natural language search across all indexed tickets, filtered to user's permissions |
| `embedding_status` | Check embedding index health and progress |
| `reembed` | Trigger re-indexing of all tickets or a specific ticket |

## Semantic Search

When semantic search is enabled, the embedder sidecar maintains a vector index of all tickets. Each ticket is chunked into a summary/metadata block and a details block, with per-action chunks added for ticket history. The embedder uses a service account with broad read access to keep the index complete.

At query time, `semantic_search` retrieves candidate chunks by vector similarity, then verifies each matching ticket against the requesting user's own HaloPSA token. Results that the user cannot access are filtered out before the response is returned. This means the index is shared but access is enforced per-user.

Supported providers:

- **local** (default) — [fastembed](https://github.com/Anush008/fastembed-rs) running in-process, no external API needed. Default model: `BAAI/bge-base-en-v1.5`.
- **openai** — Any OpenAI-compatible embeddings API. Set `HMCP_EMBED_API_KEY` and optionally override `HMCP_EMBED_API_URL` for self-hosted models.
- **ollama** — Local Ollama instance. Set `HMCP_EMBED_OLLAMA_URL`.

## OAuth Flow

The server implements a two-legged OAuth 2.1 relay:

1. The MCP client initiates authorization. The server redirects to HaloPSA's `/auth/authorize` with PKCE.
2. The user authenticates directly with HaloPSA. HaloPSA issues a code to the server's callback.
3. The server exchanges the code for HaloPSA tokens, encrypts them with AES-256-GCM, and stores them mapped to a new MCP access token.
4. Subsequent tool calls use the stored HaloPSA token. The MCP client only ever sees the MCP access token.

The server supports both hosted HaloPSA (where a tenant identifier is required) and on-premise instances (where `HMCP_HALO_TENANT` is left empty).

## Building from Source

Requires Rust 1.82 or later.

```bash
git clone https://github.com/Gumbees/halopsa-mcp
cd halopsa-mcp
cargo build --release
```

Binaries are written to `target/release/hmcp-server` and `target/release/hmcp-embedder`.

The release profile enables LTO, binary stripping, and size optimization (`opt-level = "z"`).

To build Docker images directly:

```bash
# Server image
docker build --target server -t halopsa-mcp-server .

# Embedder image
docker build --target embedder -t halopsa-mcp-embedder .
```

## Status

Phase 1 is complete: tickets, actions, workflow transitions, and semantic search. Planned for future phases: assets, clients, contracts, and invoices.

## License

MIT
