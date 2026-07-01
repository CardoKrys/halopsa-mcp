# HaloPSA MCP Server

Rust-based MCP server for HaloPSA. Enables AI assistants to interact with HaloPSA tickets, actions, and workflows through the Model Context Protocol. Users authenticate directly with HaloPSA via OAuth 2.1 relay; all API calls carry their real HaloPSA permissions.

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
- **Single replica mandatory**: OAuth in-flight state (pending_auths, auth_codes) and live SSE sessions are held in process memory. Horizontal scaling requires moving this state to Postgres first.

## Deployment Paths

### Path A — VM + Docker Compose (current production)
Docker Compose with Caddy reverse proxy.

```bash
docker compose up -d --build
```

See `INSTALL.md` for the full walkthrough.

### Path B — Azure Container Apps (target)
Bicep IaC deploys ACR + PostgreSQL Flexible Server + Container Apps. Secrets are injected at Container App startup via managed identity — no shell script needed at runtime.

```bash
./deploy.sh    # handles az login, Bicep deployment, image build, and image push
```

See `INSTALL_AZURE.md` for prerequisites and the full walkthrough.

## Building Locally

```bash
cargo build --release
```

## Dockerfile Targets

| Target | Used by | Builds |
|--------|---------|--------|
| `server` | VM Docker Compose, Azure ACR build | `hmcp-server` binary |
| `embedder` | Future Azure Container App (when semantic search enabled) | `hmcp-embedder` binary |

## Environment

See `.env.example` for all configuration options.

## Azure Infrastructure

`infra/` contains Bicep templates for the Azure Container Apps deployment:

```
infra/
├── main.bicep              — ACR, UAMI, PostgreSQL, Container Apps environment + app
├── main.parameters.json    — Non-sensitive parameters (haloUrl/clientId/tenant populated by deploy.sh from KV)
└── modules/
    ├── kv-access.bicep     — Grants managed identity Key Vault Secrets User role
    └── kv-secrets.bicep    — Writes secrets to Key Vault (write-if-provided semantics)
```

Key Vault secret names used:

| Secret | Written by | Read by |
|--------|-----------|---------|
| `halopsa-mcp-halo-url` | Manual / VM setup | `deploy.sh` → Bicep param |
| `halopsa-mcp-client-id` | Manual / VM setup | `deploy.sh` → Bicep param |
| `halopsa-mcp-tenant` | Manual / VM setup | `deploy.sh` → Bicep param |
| `halopsa-mcp-encryption-key` | `deploy.sh` on first deploy | Container App (secretRef) |
| `halopsa-mcp-halo-client-secret` | `deploy.sh` on first deploy | Container App (secretRef) |
| `halopsa-mcp-database-url` | Bicep on every deploy | Container App (secretRef) |
