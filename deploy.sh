#!/usr/bin/env bash
# deploy.sh — One-shot Azure Container Apps deployment for HaloPSA MCP.
#
# Usage: ./deploy.sh
#
# Requires: Azure CLI (az), jq
# The deploying account needs:
#   - Owner on the target resource group (to create role assignments)

set -euo pipefail

RG="${RESOURCE_GROUP:-halopsa-mcp-rg}"
LOCATION="${AZURE_LOCATION:-uksouth}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PARAMS_FILE="$SCRIPT_DIR/infra/main.parameters.json"

# ── 1. Check Azure authentication ─────────────────────────────────────────────
echo ""
if az account show --output none 2>/dev/null; then
    echo "==> Logged in as: $(az account show --query user.name -o tsv)"
else
    echo "==> Logging in to Azure..."
    az login
fi
echo ""

# Optional: pin to a specific subscription.
if [[ -n "${AZURE_SUBSCRIPTION_ID:-}" ]]; then
    echo "==> Setting subscription: $AZURE_SUBSCRIPTION_ID"
    az account set --subscription "$AZURE_SUBSCRIPTION_ID" --output none
fi

# ── 2. Read non-sensitive config from main.parameters.json ────────────────────
echo "==> Reading config from $PARAMS_FILE"

HALO_URL=$(jq -r '.parameters.haloUrl.value // ""' "$PARAMS_FILE")
HALO_CLIENT_ID=$(jq -r '.parameters.haloClientId.value // ""' "$PARAMS_FILE")
HALO_TENANT=$(jq -r '.parameters.haloTenant.value // ""' "$PARAMS_FILE")

echo "  haloUrl:      $HALO_URL"
echo "  haloClientId: $HALO_CLIENT_ID"
echo "  haloTenant:   ${HALO_TENANT:-<empty>}"
echo ""

if [[ -z "$HALO_URL" || -z "$HALO_CLIENT_ID" ]]; then
    echo "ERROR: Fill in haloUrl and haloClientId in infra/main.parameters.json."
    exit 1
fi

# ── 3. Prompt for the PostgreSQL admin password ───────────────────────────────
echo "Enter the PostgreSQL admin password."
echo "(Required on every deploy — used to compose the database connection URL.)"
read -rsp "  PostgreSQL admin password: " PG_PASSWORD
echo ""
[[ -z "$PG_PASSWORD" ]] && { echo "ERROR: PostgreSQL admin password cannot be empty."; exit 1; }
echo ""

# ── 4. Prompt for the encryption key ─────────────────────────────────────────
echo "Enter the AES-256-GCM encryption key."
echo "(Required on every deploy — store it in a password manager. NEVER change it after first deploy.)"
read -rsp "  Encryption key: " ENC_KEY
echo ""
[[ -z "$ENC_KEY" ]] && { echo "ERROR: Encryption key cannot be empty."; exit 1; }
echo ""

# ── 5. Ensure resource group exists ───────────────────────────────────────────
echo "==> Using resource group: $RG"
az group show --name "$RG" --output none 2>/dev/null || {
    echo "==> Resource group '$RG' not found. Creating in '$LOCATION'..."
    az group create --name "$RG" --location "$LOCATION" --output none
}

# ── 6. Deploy Bicep infrastructure ────────────────────────────────────────────
echo "==> Deploying infrastructure via Bicep (first run takes ~10 minutes)..."

DEPLOYMENT_OUTPUT=$(az deployment group create \
    --resource-group "$RG" \
    --template-file "$SCRIPT_DIR/infra/main.bicep" \
    --parameters "@$PARAMS_FILE" \
    --parameters \
        "haloUrl=$HALO_URL" \
        "haloClientId=$HALO_CLIENT_ID" \
        "haloTenant=$HALO_TENANT" \
        "pgAdminPassword=$PG_PASSWORD" \
        "encryptionKey=$ENC_KEY" \
    --query properties.outputs \
    --output json)

ACR_NAME=$(echo "$DEPLOYMENT_OUTPUT"    | jq -r '.acrName.value')
ACR_SERVER=$(echo "$DEPLOYMENT_OUTPUT"  | jq -r '.acrLoginServer.value')
APP_NAME=$(echo "$DEPLOYMENT_OUTPUT"    | jq -r '.containerAppName.value')
FQDN=$(echo "$DEPLOYMENT_OUTPUT"        | jq -r '.containerAppFqdn.value')

echo "  ACR:           $ACR_NAME"
echo "  Container App: $APP_NAME"
echo "  FQDN:          $FQDN"
echo ""

# ── 7. Build and push the server image to ACR ─────────────────────────────────
echo "==> Building and pushing server image to ACR..."
echo "    (Runs in Azure — no local Docker required. First build ~5 minutes.)"
az acr build \
    --registry "$ACR_NAME" \
    --image "halopsa-mcp-server:latest" \
    --target server \
    "$SCRIPT_DIR"

# ── 8. Force the Container App to pull the new image ─────────────────────────
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
