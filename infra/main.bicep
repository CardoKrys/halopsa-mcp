// HaloPSA MCP — Azure Container Apps + PostgreSQL Flexible Server deployment.
//
// Provisions, in one resource group:
//   - Azure Container Registry (for the `server` image)
//   - User-assigned managed identity (AcrPull on the registry + Key Vault Secrets
//     User on the existing HaloSecrets vault)
//   - PostgreSQL Flexible Server (public access + firewall, `vector` allowlisted)
//   - Log Analytics workspace + Container Apps environment
//   - The Container App itself (external ingress :8080, single replica)
//
// Networking model: public access + firewall (simplest path).
// See INSTALL_AZURE.md for the full deploy walkthrough using deploy.sh.

@description('Azure region for all resources.')
param location string = resourceGroup().location

@description('Base name used to derive resource names.')
param namePrefix string = 'halopsa-mcp'

@description('Globally-unique ACR name (alphanumeric, 5-50 chars, lowercase).')
param acrName string = '${replace(namePrefix, '-', '')}${uniqueString(resourceGroup().id)}'

@description('Globally-unique Flexible Server name (lowercase, 3-63 chars).')
param pgServerName string = '${namePrefix}-pg-${uniqueString(resourceGroup().id)}'

@description('PostgreSQL administrator login.')
param pgAdminLogin string = 'hmcpadmin'

@description('PostgreSQL administrator password.')
@secure()
param pgAdminPassword string

@description('Application database name.')
param dbName string = 'hmcp'

@description('Name of the existing Key Vault holding the halopsa-mcp-* secrets.')
param keyVaultName string = 'HaloSecrets'

@description('Resource group of the existing Key Vault (defaults to this RG).')
param keyVaultResourceGroup string = resourceGroup().name

@description('Key Vault secret name holding the HaloPSA OAuth client secret.')
param clientSecretSecretName string = 'halopsa-mcp-halo-client-secret'

@description('Key Vault secret name holding the AES-256-GCM encryption key.')
param encryptionKeySecretName string = 'halopsa-mcp-encryption-key'

@description('Key Vault secret name holding the full Postgres connection URL.')
param databaseUrlSecretName string = 'halopsa-mcp-database-url'

@description('HaloPSA OAuth client secret. Provide to (re)write it into Key Vault; leave empty on redeploys to preserve the existing value.')
@secure()
param haloClientSecret string = ''

@description('AES-256-GCM encryption key (32+ chars). Provide ONCE on the first deploy; leave empty on every redeploy so it is never overwritten (changing it invalidates all stored tokens).')
@secure()
param encryptionKey string = ''

@description('HaloPSA instance URL, e.g. https://psa.example.com')
param haloUrl string

@description('HaloPSA OAuth Application client ID (Authorization Code flow).')
param haloClientId string

@description('HaloPSA tenant (hosted instances only; empty for on-premise).')
param haloTenant string = ''

@description('Enable semantic search. Leave false unless the embedder is deployed.')
param semanticSearch bool = false

@description('Container image tag to run (push to ACR as halopsa-mcp-server:<tag>).')
param imageTag string = 'latest'

var containerAppName = namePrefix
var imageName = 'halopsa-mcp-server'

// Built-in role definition IDs.
var acrPullRoleId = '7f951dda-4ed3-4680-a7ca-43fe172d538d'
var kvSecretsUserRoleId = '4633458b-17de-408a-b874-0445c86b69e6'

resource acr 'Microsoft.ContainerRegistry/registries@2023-07-01' = {
  name: acrName
  location: location
  sku: {
    name: 'Basic'
  }
  properties: {
    adminUserEnabled: false
  }
}

resource uami 'Microsoft.ManagedIdentity/userAssignedIdentities@2023-01-31' = {
  name: '${namePrefix}-id'
  location: location
}

// Let the Container App pull the image using the managed identity.
resource acrPull 'Microsoft.Authorization/roleAssignments@2022-04-01' = {
  name: guid(acr.id, uami.id, acrPullRoleId)
  scope: acr
  properties: {
    roleDefinitionId: subscriptionResourceId('Microsoft.Authorization/roleDefinitions', acrPullRoleId)
    principalId: uami.properties.principalId
    principalType: 'ServicePrincipal'
  }
}

// Existing Key Vault (may live in another resource group).
resource keyVault 'Microsoft.KeyVault/vaults@2023-07-01' existing = {
  name: keyVaultName
  scope: resourceGroup(keyVaultResourceGroup)
}

// Grant the managed identity read access to the vault's secrets (cross-RG safe).
module kvAccess 'modules/kv-access.bicep' = {
  name: 'kv-secrets-user'
  scope: resourceGroup(keyVaultResourceGroup)
  params: {
    keyVaultName: keyVaultName
    principalId: uami.properties.principalId
    roleDefinitionId: kvSecretsUserRoleId
  }
}

resource pg 'Microsoft.DBforPostgreSQL/flexibleServers@2023-06-01-preview' = {
  name: pgServerName
  location: location
  sku: {
    name: 'Standard_B1ms'
    tier: 'Burstable'
  }
  properties: {
    administratorLogin: pgAdminLogin
    administratorLoginPassword: pgAdminPassword
    version: '16'
    storage: {
      storageSizeGB: 32
    }
    backup: {
      backupRetentionDays: 7
      geoRedundantBackup: 'Disabled'
    }
    highAvailability: {
      mode: 'Disabled'
    }
    network: {
      publicNetworkAccess: 'Enabled'
    }
  }
}

// Allowlist the pgvector extension. Only consumed when HMCP_SEMANTIC_SEARCH=true,
// but allowlisting is harmless otherwise. An admin must still run
// `CREATE EXTENSION IF NOT EXISTS vector;` once in the target database.
resource pgExtensions 'Microsoft.DBforPostgreSQL/flexibleServers/configurations@2023-06-01-preview' = {
  parent: pg
  name: 'azure.extensions'
  properties: {
    value: 'VECTOR'
    source: 'user-override'
  }
}

// "Allow public access from any Azure service within Azure to this server."
// The Container App's egress IPs are not static on a non-VNet environment, so
// this is the simplest way to let it connect. Tighten or replace with a private
// endpoint when moving to the VNet-integrated model.
resource pgFirewallAzure 'Microsoft.DBforPostgreSQL/flexibleServers/firewallRules@2023-06-01-preview' = {
  parent: pg
  name: 'AllowAllAzureServicesAndResourcesWithinAzureIps'
  properties: {
    startIpAddress: '0.0.0.0'
    endIpAddress: '0.0.0.0'
  }
}

resource pgDatabase 'Microsoft.DBforPostgreSQL/flexibleServers/databases@2023-06-01-preview' = {
  parent: pg
  name: dbName
  properties: {
    charset: 'UTF8'
    collation: 'en_US.utf8'
  }
}

// Write the secrets into the existing vault (write-if-provided; see module).
// The DB URL is composed inside the module so the password stays a secure value.
// The deploying principal needs `Key Vault Secrets Officer` on the vault.
module kvSecrets 'modules/kv-secrets.bicep' = {
  name: 'kv-secrets'
  scope: resourceGroup(keyVaultResourceGroup)
  params: {
    keyVaultName: keyVaultName
    clientSecretSecretName: clientSecretSecretName
    encryptionKeySecretName: encryptionKeySecretName
    databaseUrlSecretName: databaseUrlSecretName
    haloClientSecret: haloClientSecret
    encryptionKey: encryptionKey
    pgAdminLogin: pgAdminLogin
    pgAdminPassword: pgAdminPassword
    pgFqdn: pg.properties.fullyQualifiedDomainName
    dbName: dbName
  }
}

resource law 'Microsoft.OperationalInsights/workspaces@2023-09-01' = {
  name: '${namePrefix}-logs'
  location: location
  properties: {
    sku: {
      name: 'PerGB2018'
    }
    retentionInDays: 30
  }
}

resource env 'Microsoft.App/managedEnvironments@2024-03-01' = {
  name: '${namePrefix}-env'
  location: location
  properties: {
    appLogsConfiguration: {
      destination: 'log-analytics'
      logAnalyticsConfiguration: {
        customerId: law.properties.customerId
        sharedKey: law.listKeys().primarySharedKey
      }
    }
  }
}

// Ingress FQDN is deterministic once the environment exists, so we can feed it
// to HMCP_PUBLIC_DOMAIN in the same deployment. Register
// https://<this>/callback as the redirect URI on the HaloPSA OAuth Application.
var publicDomain = '${containerAppName}.${env.properties.defaultDomain}'

resource app 'Microsoft.App/containerApps@2024-03-01' = {
  name: containerAppName
  location: location
  identity: {
    type: 'UserAssigned'
    userAssignedIdentities: {
      '${uami.id}': {}
    }
  }
  properties: {
    managedEnvironmentId: env.id
    configuration: {
      activeRevisionsMode: 'Single'
      ingress: {
        external: true
        targetPort: 8080
        transport: 'auto'
        allowInsecure: false
      }
      registries: [
        {
          server: acr.properties.loginServer
          identity: uami.id
        }
      ]
      secrets: [
        {
          name: 'halo-client-secret'
          keyVaultUrl: '${keyVault.properties.vaultUri}secrets/${clientSecretSecretName}'
          identity: uami.id
        }
        {
          name: 'encryption-key'
          keyVaultUrl: '${keyVault.properties.vaultUri}secrets/${encryptionKeySecretName}'
          identity: uami.id
        }
        {
          name: 'database-url'
          keyVaultUrl: '${keyVault.properties.vaultUri}secrets/${databaseUrlSecretName}'
          identity: uami.id
        }
      ]
    }
    template: {
      containers: [
        {
          name: imageName
          image: '${acr.properties.loginServer}/${imageName}:${imageTag}'
          resources: {
            cpu: json('0.5')
            memory: '1Gi'
          }
          env: [
            {
              name: 'HMCP_HALO_URL'
              value: haloUrl
            }
            {
              name: 'HMCP_HALO_CLIENT_ID'
              value: haloClientId
            }
            {
              name: 'HMCP_HALO_TENANT'
              value: haloTenant
            }
            {
              name: 'HMCP_HALO_CLIENT_SECRET'
              secretRef: 'halo-client-secret'
            }
            {
              name: 'HMCP_ENCRYPTION_KEY'
              secretRef: 'encryption-key'
            }
            {
              name: 'HMCP_DATABASE_URL'
              secretRef: 'database-url'
            }
            {
              name: 'HMCP_PUBLIC_DOMAIN'
              value: publicDomain
            }
            {
              name: 'HMCP_HOST'
              value: '0.0.0.0'
            }
            {
              name: 'HMCP_PORT'
              value: '8080'
            }
            {
              name: 'HMCP_SEMANTIC_SEARCH'
              value: string(semanticSearch)
            }
          ]
        }
      ]
      scale: {
        // Single replica is REQUIRED: OAuth in-flight state (pending_auths,
        // auth_codes) and live SSE sessions are held in process memory, not in
        // Postgres. A second replica would drop in-flight authorizations.
        minReplicas: 1
        maxReplicas: 1
      }
    }
  }
  // The first revision needs: the secrets present in the vault (kvSecrets), the
  // MI's read role propagated (kvAccess), and image-pull rights (acrPull).
  dependsOn: [
    kvSecrets
    kvAccess
    acrPull
  ]
}

output containerAppFqdn string = app.properties.configuration.ingress.fqdn
output containerAppName string = containerAppName
output publicDomain string = publicDomain
output acrLoginServer string = acr.properties.loginServer
output acrName string = acr.name
output postgresFqdn string = pg.properties.fullyQualifiedDomainName
output identityPrincipalId string = uami.properties.principalId
