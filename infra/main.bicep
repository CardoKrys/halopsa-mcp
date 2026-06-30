// HaloPSA MCP — Azure Container Apps + PostgreSQL Flexible Server deployment.
//
// No Key Vault dependency. Configuration is passed as plain environment variables
// or Container App inline secrets (encrypted at rest in Azure).
//
// Provisions, in one resource group:
//   - Azure Container Registry (for the `server` image)
//   - User-assigned managed identity (AcrPull on the registry)
//   - PostgreSQL Flexible Server (public access + firewall, `vector` allowlisted)
//   - Log Analytics workspace + Container Apps environment
//   - The Container App itself (external ingress :8080, single replica)
//
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

@description('AES-256-GCM encryption key (32+ chars). Required on every deploy — store in a password manager.')
@secure()
param encryptionKey string

@description('HaloPSA instance URL, e.g. https://halosb.cardonet.com')
param haloUrl string

@description('HaloPSA OAuth Application client ID (Authorization Code flow).')
param haloClientId string

@description('HaloPSA tenant (hosted instances only; empty for on-premise).')
param haloTenant string = ''

@description('Enable semantic search. Leave false unless the embedder is deployed.')
param semanticSearch bool = false

@description('Container image tag to run.')
param imageTag string = 'latest'

var containerAppName = namePrefix
var imageName = 'halopsa-mcp-server'
var acrPullRoleId = '7f951dda-4ed3-4680-a7ca-43fe172d538d'
var databaseUrl = 'postgres://${pgAdminLogin}:${pgAdminPassword}@${pg.properties.fullyQualifiedDomainName}:5432/${dbName}?sslmode=require'

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

// Allowlist the pgvector extension (harmless when semanticSearch=false).
// An admin must still run `CREATE EXTENSION IF NOT EXISTS vector;` once if enabling semantic search.
resource pgExtensions 'Microsoft.DBforPostgreSQL/flexibleServers/configurations@2023-06-01-preview' = {
  parent: pg
  name: 'azure.extensions'
  properties: {
    value: 'VECTOR'
    source: 'user-override'
  }
}

// Allows all Azure-egress IPs to reach Postgres. Container App egress IPs are
// not static without VNet integration — this is the simplest approach for now.
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
      // Inline secrets — encrypted at rest in Azure, no Key Vault required.
      // encryptionKey and databaseUrl are sensitive; everything else is a plain env var.
      secrets: [
        {
          name: 'encryption-key'
          value: encryptionKey
        }
        {
          name: 'database-url'
          value: databaseUrl
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
            { name: 'HMCP_HALO_URL', value: haloUrl }
            { name: 'HMCP_HALO_CLIENT_ID', value: haloClientId }
            { name: 'HMCP_HALO_TENANT', value: haloTenant }
            { name: 'HMCP_HALO_CLIENT_SECRET', value: '' }
            { name: 'HMCP_ENCRYPTION_KEY', secretRef: 'encryption-key' }
            { name: 'HMCP_DATABASE_URL', secretRef: 'database-url' }
            { name: 'HMCP_PUBLIC_DOMAIN', value: publicDomain }
            { name: 'HMCP_HOST', value: '0.0.0.0' }
            { name: 'HMCP_PORT', value: '8080' }
            { name: 'HMCP_SEMANTIC_SEARCH', value: string(semanticSearch) }
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
  dependsOn: [
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
