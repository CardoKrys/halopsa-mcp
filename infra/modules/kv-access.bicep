// Grants a principal a built-in role on an existing Key Vault. Deployed at the
// vault's resource group scope so it works even when the vault lives in a
// different RG than the rest of the deployment.

@description('Name of the existing Key Vault.')
param keyVaultName string

@description('Principal (managed identity) object ID to grant access to.')
param principalId string

@description('Built-in role definition GUID (default: Key Vault Secrets User).')
param roleDefinitionId string = '4633458b-17de-408a-b874-0445c86b69e6'

resource keyVault 'Microsoft.KeyVault/vaults@2023-07-01' existing = {
  name: keyVaultName
}

resource roleAssignment 'Microsoft.Authorization/roleAssignments@2022-04-01' = {
  name: guid(keyVault.id, principalId, roleDefinitionId)
  scope: keyVault
  properties: {
    roleDefinitionId: subscriptionResourceId('Microsoft.Authorization/roleDefinitions', roleDefinitionId)
    principalId: principalId
    principalType: 'ServicePrincipal'
  }
}
