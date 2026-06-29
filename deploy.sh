#!/usr/bin/env bash
# deploy.sh — One-shot Azure Container Apps deployment for HaloPSA MCP.
#
# Usage: ./deploy.sh
#
# On first deploy: supply the encryption key and (optionally) the HaloPSA
# OAuth client secret when prompted.
# On redeploys: press Enter at those two prompts to keep the existing KV values.
#
# Requires: Azure CLI (az), jq
# The deploying account needs:
#   - Contributor (or Owner) on the target subscription/resource group
#   - Key Vault Secrets Officer on the HaloSecrets vault

set -euo pipefail

VAULT="${KEYVAULT_NAME:-HaloSecrets}"
RG="${RESOURCE_GROUP:-halopsa-mcp-rg}"
LOCATION="${AZURE_LOCATION:-uksouth}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PARAMS_FILE="$SCRIPT_DIR/infra/main.parameters.json"

# ── 1. Interactive Azure login ────────────────────────────────────────────────
echo ""
echo "==> Logging in to Azure (a browser window will open)..."
az login
echo ""

# Optional: pin to a specific subscription.
if [[ -n "${AZURE_SUBSCRIPTION_ID:-}" ]]; then
    echo "==> Setting subscription: $AZURE_SUBSCRIPTION_ID"
    az account set --subscription "$AZURE_SUBSCRIPTION_ID" --output none
fi

# ── 2. Read non-sensitive config from main.parameters.json ────────────────────
# haloUrl, haloClientId, and haloTenant are the LIVE instance values.
# They live in infra/main.parameters.json — not in Key Vault — because the vault
# already holds sandbox values under the same secret names and we must not mix them.
echo "==> Reading live instance config from $PARAMS_FILE"

HALO_URL=$(jq -r '.parameters.haloUrl.value // ""' "$PARAMS_FILE")
HALO_CLIENT_ID=$(jq -r '.parameters.haloClientId.value // ""' "$PARAMS_FILE")
HALO_TENANT=$(jq -r '.parameters.haloTenant.value // ""' "$PARAMS_FILE")

echo "  haloUrl:      $HALO_URL"
echo "  haloClientId: $HALO_CLIENT_ID"
echo "  haloTenant:   ${HALO_TENANT:-<empty>}"
echo ""

if [[ -z "$HALO_URL" || "$HALO_URL" == "REPLACE_WITH_LIVE_URL" \
   || -z "$HALO_CLIENT_ID" || "$HALO_CLIENT_ID" == "REPLACE_WITH_LIVE_CLIENT_ID" ]]; then
    echo "ERROR: Fill in haloUrl and haloClientId in infra/main.parameters.json"
    echo "       before running deploy.sh for the live instance."
    exit 1
fi

# ── 3. Prompt for the PostgreSQL admin password (required every run) ──────────
# Bicep uses this to compose the database URL written into Key Vault.
echo "Enter the PostgreSQL admin password."
echo "(Required on every deploy — used to compose the database connection URL.)"
read -rsp "  PostgreSQL admin password: " PG_PASSWORD
echo ""
[[ -z "$PG_PASSWORD" ]] && { echo "ERROR: PostgreSQL admin password cannot be empty."; exit 1; }
echo ""

# ── 4. Prompt for first-deploy-only secrets ───────────────────────────────────
# Press Enter on redeploys — Bicep's write-if-provided logic preserves the
# existing Key Vault values when the parameter is empty.
echo "First-deploy only — press Enter on redeploys to keep the existing KV value."
echo ""
read -rsp "  AES-256-GCM encryption key (32+ chars, NEVER change after first deploy): " ENC_KEY
echo ""
read -rsp "  HaloPSA OAuth client secret (blank = PKCE / no client secret): " CLIENT_SECRET
echo ""
echo ""

# ── 5. Create resource group ──────────────────────────────────────────────────
echo "==> Creating resource group '$RG' in '$LOCATION' (idempotent)..."
az group create --name "$RG" --location "$LOCATION" --output none

# ── 6. Deploy Bicep infrastructure ────────────────────────────────────────────
echo "==> Deploying infrastructure via Bicep (first run takes ~10 minutes)..."

EXTRA_PARAMS=()
[[ -n "$ENC_KEY" ]]       && EXTRA_PARAMS+=(-p "encryptionKey=$ENC_KEY")
[[ -n "$CLIENT_SECRET" ]] && EXTRA_PARAMS+=(-p "haloClientSecret=$CLIENT_SECRET")

DEPLOYMENT_OUTPUT=$(az deployment group create \
    --resource-group "$RG" \
    --template-file "$SCRIPT_DIR/infra/main.bicep" \
    --parameters "@$PARAMS_FILE" \
    --parameters \
        "haloUrl=$HALO_URL" \
        "haloClientId=$HALO_CLIENT_ID" \
        "haloTenant=$HALO_TENANT" \
        "pgAdminPassword=$PG_PASSWORD" \
    "${EXTRA_PARAMS[@]+"${EXTRA_PARAMS[@]}"}" \
    --query properties.outputs \
    --output json)

ACR_NAME=$(echo "$DEPLOYMENT_OUTPUT"    | jq -r '.acrName.value')
ACR_SERVER=$(echo "$DEPLOYMENT_OUTPUT"  | jq -r '.acrLoginServer.value')
APP_NAME=$(echo "$DEPLOYMENT_OUTPUT"    | jq -r '.containerAppName.value')
FQDN=$(echo "$DEPLOYMENT_OUTPUT"        | jq -r '.containerAppFqdn.value')

echo "  ACR:              $ACR_NAME"
echo "  Container App:    $APP_NAME"
echo "  FQDN:             $FQDN"
echo ""

# ── 7. Build and push the server image to ACR ─────────────────────────────────
# az acr build runs the Docker build in Azure (no local Docker required).
# --target server means only the server stage of the Dockerfile is built and run;
# the embedder stage is never executed.
echo "==> Building and pushing server image to ACR..."
echo "    (This runs in Azure — no local Docker required. First build ~5 minutes.)"
az acr build \
    --registry "$ACR_NAME" \
    --image "halopsa-mcp-server:latest" \
    --target server \
    "$SCRIPT_DIR"

# ── 8. Force the Container App to pull the new image ─────────────────────────
# Azure Container Apps do not automatically re-pull on a tag push. We update the
# app with the same image reference, which triggers a new revision and a fresh pull.
echo ""
echo "==> Updating Container App to pull the new image..."
az containerapp update \
    --name "$APP_NAME" \
    --resource-group "$RG" \
    --image "$ACR_SERVER/halopsa-mcp-server:latest" \
    --output none

# ── 9. Summary ────────────────────────────────────────────────────────────────
echo ""
echo "============================================================"
echo " Deployment complete!"
echo "============================================================"
echo ""
echo " Container App URL : https://$FQDN"
echo " MCP SSE endpoint  : https://$FQDN/mcp/sse"
echo " OAuth callback    : https://$FQDN/callback"
echo " Health check      : https://$FQDN/health"
echo ""
echo " ACTION REQUIRED (first deploy only):"
echo " Register the OAuth callback URI in your HaloPSA OAuth Application:"
echo "   https://$FQDN/callback"
echo ""
echo " Allow 1-2 minutes for the container to fully start, then:"
echo "   curl -I https://$FQDN/health"
echo "============================================================"
