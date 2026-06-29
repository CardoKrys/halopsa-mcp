# Azure Container Apps — Deployment Guide

This deploys the HaloPSA MCP server to Azure Container Apps using `deploy.sh` as a single entry point. Run it once to provision everything; run it again to redeploy after code changes.

---

## What you need right now

| What | Where to get it |
|------|----------------|
| Live HaloPSA URL + OAuth client ID | Create the OAuth Application first (Step 1) |
| PostgreSQL admin password | Make one up — write it down, you need it on every redeploy |
| Encryption key | Generate with `openssl rand -base64 32` — save it permanently, never change it |
| A terminal with `az` and `jq` | Azure Cloud Shell is easiest (already has both, already logged in) |

---

## Before you run the script

### Step 1 — Create the OAuth Application in your live HaloPSA instance

You need the client ID before running the deploy.

1. In your live HaloPSA, go to **Configuration → Integrations → API**
2. Create a new application:
   - Grant type: **Authorization Code**
   - Client secret: **leave blank** (PKCE flow — no secret needed)
   - Redirect URI: leave blank for now — you don't have the Container App URL yet
3. Copy the **Client ID**

### Step 2 — Fill in `infra/main.parameters.json`

Open the file and replace the two placeholders:

```json
"haloUrl":      { "value": "https://<your-live-halo-url>" },
"haloClientId": { "value": "<client-id-from-step-1>" }
```

If your live instance is **hosted HaloPSA**, also set `haloTenant` to your tenant name.
If it is **on-premise**, leave `haloTenant` as `""`.

### Step 3 — Prepare your two secret values

Generate an encryption key now and save it somewhere permanent (password manager, etc.):

```bash
openssl rand -base64 32
```

> **Critical:** Once the first deploy runs, this key encrypts all stored HaloPSA tokens.
> If you lose it or change it, every user will have to re-authenticate.
> Never change it after the first deploy.

The PostgreSQL admin password is less critical but must stay consistent across redeploys — you'll be prompted for it every time.

---

## Running the deploy

### Step 4 — Open a terminal

**Easiest on Windows:** Use [Azure Cloud Shell](https://shell.azure.com) in a browser. It has `az` and `jq` pre-installed and you're already authenticated — skip the `az login` step.

Alternatively: WSL or Git Bash with Azure CLI installed.

### Step 5 — Run the script

```bash
cd mcp_fixed_vm
chmod +x deploy.sh
./deploy.sh
```

When prompted:
- **PostgreSQL admin password** — enter the password you chose
- **AES-256-GCM encryption key** — paste the key you generated in Step 3
- **HaloPSA OAuth client secret** — press **Enter** (blank = PKCE, no secret needed)

The script will:
1. Log you in to Azure (opens a browser — skip if using Cloud Shell)
2. Read the live HaloPSA URL and client ID from `infra/main.parameters.json`
3. Create the resource group `halopsa-mcp-rg` in `uksouth`
4. Deploy all Azure infrastructure via Bicep (~10 minutes first run)
5. Build the Docker image in the cloud via `az acr build` (~5 minutes, no local Docker needed)
6. Start the Container App and print the URL

At the end you'll see something like:

```
============================================================
 Deployment complete!
============================================================

 Container App URL : https://halopsa-mcp.<hash>.uksouth.azurecontainerapps.io
 MCP SSE endpoint  : https://halopsa-mcp.<hash>.uksouth.azurecontainerapps.io/mcp/sse
 OAuth callback    : https://halopsa-mcp.<hash>.uksouth.azurecontainerapps.io/callback
 Health check      : https://halopsa-mcp.<hash>.uksouth.azurecontainerapps.io/health
============================================================
```

---

## After the deploy

### Step 6 — Register the OAuth callback in live HaloPSA

Go back to the OAuth Application you created in Step 1 and add the callback URL as the **Redirect URI**:

```
https://halopsa-mcp.<hash>.uksouth.azurecontainerapps.io/callback
```

### Step 7 — Verify it's running

Allow 1–2 minutes for the container to fully start, then:

```bash
curl -I https://<fqdn>/health
# Expected: HTTP/2 200
```

### Step 8 — Connect in Claude.ai

Add a custom connector with the MCP SSE URL:

```
https://<fqdn>/mcp/sse
```

---

## Redeploys (after code changes)

```bash
./deploy.sh
```

At the prompts:
- **PostgreSQL admin password** — enter the same password as before
- **Encryption key** — press **Enter** (keep existing)
- **OAuth client secret** — press **Enter** (keep existing)

The script rebuilds the image, pushes it, and restarts the Container App. Downtime is ~30 seconds during the revision switch.

---

## Azure resources created

All resources land in `halopsa-mcp-rg`:

| Resource | Type | Notes |
|----------|------|-------|
| `halopsamcp<hash>` | Container Registry (Basic) | Stores the server image |
| `halopsa-mcp-id` | User-assigned Managed Identity | AcrPull + KV Secrets User |
| `halopsa-mcp-pg-<hash>` | PostgreSQL Flexible Server (B1ms) | Managed Postgres, 32 GB |
| `halopsa-mcp-logs` | Log Analytics Workspace | 30-day retention |
| `halopsa-mcp-env` | Container Apps Environment | |
| `halopsa-mcp` | Container App | Single replica, HTTPS ingress |

New secrets written to HaloSecrets vault on first deploy (all prefixed `live-` to avoid collision with sandbox):

| Secret | Content |
|--------|---------|
| `halopsa-mcp-live-halo-client-secret` | OAuth client secret (empty for PKCE) |
| `halopsa-mcp-live-encryption-key` | AES-256-GCM key |
| `halopsa-mcp-live-database-url` | Postgres connection URL (rewritten on every deploy) |

---

## Constraints

| Constraint | Detail |
|-----------|--------|
| **Single replica only** | OAuth in-flight state and SSE sessions are in process memory. Never scale above 1 replica without first moving that state to Postgres. |
| **Encryption key is immutable** | Set on first deploy only. Changing it makes all stored tokens undecryptable. |
| **Postgres firewall** | Allows all Azure egress (`0.0.0.0` rule). Container App egress IPs are not static without VNet integration — this is the simplest approach for now. |
| **Semantic search disabled** | `semanticSearch: false` by default. The embedder is not deployed. Enable when ready by setting it to `true` in `main.parameters.json` and deploying a separate embedder Container App. |
