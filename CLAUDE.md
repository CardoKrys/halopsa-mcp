# HaloPSA MCP Server

Rust-based MCP server for HaloPSA. Enables AI assistants to interact with HaloPSA tickets, actions, and workflows through the Model Context Protocol.

## Architecture

Cargo workspace with 4 crates:

| Crate | Type | Purpose |
|-------|------|---------|
| `hmcp-common` | library | Shared types, HaloPSA API client, DB traits, ticket chunking |
| `hmcp-db-postgres` | library | Postgres + pgvector backend (auth tokens + semantic embeddings) |
| `hmcp-server` | binary | MCP server (SSE + Streamable HTTP transport, OAuth 2.1 relay, tools) |
| `hmcp-embedder` | binary | Embedding sidecar (fastembed/OpenAI/Ollama, job queue worker) |

## Key Design Decisions

- **OAuth relay**: MCP server relays OAuth to HaloPSA's own auth. Users authenticate directly with HaloPSA, so all API calls carry their real permissions.
- **Permission-respecting semantic search**: Embeddings indexed via service account, but results filtered per-user by checking each ticket against the user's HaloPSA token.
- **Workflow-aware actions**: Available ticket actions determined by walking the ticket's workflow definition and filtering to the current step.
- **Modeled after bookstack-mcp**: Same patterns for MCP protocol, SSE transport, DB traits, embedding pipeline.

## Building

```bash
cargo build --release
```

## Environment

See `.env.example` for all configuration options.
