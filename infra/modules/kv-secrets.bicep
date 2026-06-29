// Writes the halopsa-mcp-* secrets into an existing Key Vault. Deployed at the
// vault's resource group scope so it works even when the vault lives in a
// different RG than the rest of the deployment.
//
// Write-if-provided semantics: a secret is (re)written only when its value is
// passed non-empty. On a redeploy you leave the value blank and the existing
// secret is preserved untouched — Bicep cannot read a secret to test existence,
// so this is the safe equivalent of "create if absent". The encryption key in
// particular MUST only be supplied on the first deploy; changing it later makes
// all stored HaloPSA tokens undecryptable.

@description('Name of the existing Key Vault.')
param keyVaultName string

@description('Secret name for the HaloPSA OAuth client secret.')
param clientSecretSecretName string

@description('Secret name for the AES-256-GCM encryption key.')
param encryptionKeySecretName string

@description('Secret name for the full Postgres connection URL.')
param databaseUrlSecretName string

@description('HaloPSA OAuth client secret. Empty = leave any existing value untouched.')
@secure()
param haloClientSecret string = ''

@description('AES-256-GCM encryption key (32+ chars). Provide ONCE on first deploy; leave empty on redeploys so it is never overwritten.')
@secure()
param encryptionKey string = ''

@description('Postgres administrator login (for composing the connection URL).')
param pgAdminLogin string

@description('Postgres administrator password (for composing the connection URL).')
@secure()
param pgAdminPassword string

@description('Postgres server fully-qualified domain name.')
param pgFqdn string

@description('Application database name.')
param dbName string

// Composed here so the password is only ever handled as a secure value, never
// surfaced through a plain variable in the calling template.
var databaseUrl = 'postgres://${pgAdminLogin}:${pgAdminPassword}@${pgFqdn}:5432/${dbName}?sslmode=require'

resource keyVault 'Microsoft.KeyVault/vaults@2023-07-01' existing = {
  name: keyVaultName
}

resource clientSecret 'Microsoft.KeyVault/vaults/secrets@2023-07-01' = if (!empty(haloClientSecret)) {
  parent: keyVault
  name: clientSecretSecretName
  properties: {
    value: haloClientSecret
  }
}

resource encKey 'Microsoft.KeyVault/vaults/secrets@2023-07-01' = if (!empty(encryptionKey)) {
  parent: keyVault
  name: encryptionKeySecretName
  properties: {
    value: encryptionKey
  }
}

// The DB URL is derived from the (always-required) Postgres admin password and
// the server FQDN, so it is safe to (re)write on every deploy.
resource dbUrl 'Microsoft.KeyVault/vaults/secrets@2023-07-01' = {
  parent: keyVault
  name: databaseUrlSecretName
  properties: {
    value: databaseUrl
  }
}
