# Deployment Guide (Cardonet)

This guide covers deploying the HaloPSA MCP server on an Azure Ubuntu VM with HTTPS via Caddy. It reflects the tested Cardonet configuration as of June 2026.

> **Note:** This fork disables the embedder sidecar (semantic search) due to a build incompatibility with the current ONNX Runtime crate. The core MCP server — tickets, actions, workflows, reference data — is fully functional. Semantic search can be revisited when the upstream dependency is resolved.

---

## Prerequisites

- Azure VM running Ubuntu 24.04
- Public IP with a DNS label assigned (e.g. `your-label.westeurope.cloudapp.azure.com`)
- Ports 80 and 443 open in the Network Security Group
- SSH access to the VM
- System-assigned managed identity on the VM with **Key Vault Secrets User** role on the `HaloSecrets` vault
- A HaloPSA instance with API access
- A Claude.ai account with custom connector support

---

## 1. Azure Setup

### DNS Label

1. In the Azure portal, go to your resource group
2. Open the **Public IP address** resource for your VM
3. Go to **Configuration** and set a DNS name label
4. Save — your VM will be reachable at `<label>.<region>.cloudapp.azure.com`

### Network Security Group

Add two inbound rules to your NSG:

| Name | Port | Protocol | Action | Priority |
|------|------|----------|--------|----------|
| Allow-HTTP | 80 | TCP | Allow | 310 |
| Allow-HTTPS | 443 | TCP | Allow | 320 |

### Managed Identity

1. Go to your VM in the Azure portal
2. Under **Security** → **Identity**, enable **System assigned** managed identity
3. Save, then go to the `HaloSecrets` Key Vault
4. Under **Access control (IAM)**, add a role assignment:
   - Role: **Key Vault Secrets User**
   - Member: your VM's managed identity

---

## 2. Key Vault Secrets

Ask your Key Vault administrator to create the following secrets in the `HaloSecrets` vault:

| Secret name | Value |
|-------------|-------|
| `halopsa-mcp-halo-url` | HaloPSA instance URL, e.g. `https://psa.example.com` |
| `halopsa-mcp-client-id` | Client ID from the HaloPSA OAuth application |
| `halopsa-mcp-tenant` | Tenant name for hosted HaloPSA; empty string for on-premise |
| `halopsa-mcp-encryption-key` | Random 32+ character string (`openssl rand -base64 32`) |
| `halopsa-mcp-public-domain` | VM domain, e.g. `your-label.westeurope.cloudapp.azure.com` |
| `halopsa-mcp-db-password` | Strong password for the Postgres database |

There is no client secret — the HaloPSA OAuth application uses Authorization Code + PKCE.

---

## 3. Install Docker

SSH into the VM and run the following:

```bash
sudo apt update && sudo apt upgrade -y

sudo apt install -y ca-certificates curl gnupg
sudo install -m 0755 -d /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor -o /etc/apt/keyrings/docker.gpg
sudo chmod a+r /etc/apt/keyrings/docker.gpg

echo \
  "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu \
  $(. /etc/os-release && echo "$VERSION_CODENAME") stable" | \
  sudo tee /etc/apt/sources.list.d/docker.list > /dev/null

sudo apt update && sudo apt install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin

sudo systemctl start docker
sudo systemctl enable docker
sudo usermod -aG docker $USER
newgrp docker
```

Install the Azure CLI (needed for Key Vault access):

```bash
curl -sL https://aka.ms/InstallAzureCLIDeb | sudo bash
```

Verify:

```bash
docker --version && docker compose version && az --version
```

---

## 4. HaloPSA OAuth Application

In your HaloPSA instance:

1. Go to **Configuration** → **Integrations** → **Halo API**
2. Create a new application:
   - **Name**: anything descriptive (e.g. `Claude MCP`)
   - **Auth method**: `Authorization Code`
   - **Login Redirect URI**: `https://<your-domain>/callback`
   - **Permissions**: grant appropriate API permissions
3. Note the **Client ID** — there is no client secret for Authorization Code + PKCE

---

## 5. Clone, Fetch Secrets and Start

```bash
git clone -b development https://github.com/CardoKrys/halopsa-mcp.git
cd halopsa-mcp
chmod +x fetch-secrets.sh
./fetch-secrets.sh
docker volume create hmcp-postgres
docker compose up -d --build
```

The first build compiles Rust from source and takes approximately 5-10 minutes. Subsequent starts are fast.

If your public domain differs from what is stored in Key Vault (e.g. a new VM with a different DNS label), override it:

```bash
./fetch-secrets.sh --domain your-new-label.westeurope.cloudapp.azure.com
```

Verify all containers are running:

```bash
docker compose ps
```

All three services (`postgres`, `hmcp-server`, `caddy`) should show as `Up`. Caddy obtains an SSL certificate automatically on first start.

Check the server is healthy:

```bash
curl -I https://<your-domain>/health
```

Expected response: `HTTP/2 200`

---

## 6. Connect to Claude.ai

1. Go to **claude.ai** → **Settings** → **Integrations**
2. Add a custom connector:
   - **Name**: `HaloPSA`
   - **Remote MCP server URL**: `https://<your-domain>/mcp/sse`
   - **OAuth Client ID**: your HaloPSA Client ID
   - **OAuth Client Secret**: leave blank
3. Save — Claude will redirect to HaloPSA for login
4. Authenticate with your HaloPSA credentials

Each user authenticates individually with their own HaloPSA account. All API calls carry that user's permissions.

---

## Endpoints

| Endpoint | Purpose |
|----------|---------| 
| `/health` | Health check |
| `/mcp/sse` | MCP transport (SSE and Streamable HTTP) |
| `/authorize` | OAuth authorisation |
| `/callback` | OAuth callback |
| `/token` | OAuth token exchange |
| `/.well-known/oauth-authorization-server` | OAuth server metadata |

---

## Maintenance

**View logs:**
```bash
cd ~/halopsa-mcp && docker compose logs -f hmcp-server
```

**Restart the stack:**
```bash
docker compose restart
```

**Stop the stack:**
```bash
docker compose down
```

**Update to latest:**
```bash
git pull origin development
docker compose up -d --build
```

**Rotate secrets:** re-run `./fetch-secrets.sh` then `docker compose restart hmcp-server`.
