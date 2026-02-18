/**
 * GraphQL queries and mutations for configuration management.
 */

export const GET_CONFIG = `
  query GetConfig {
    config {
      providers {
        name
        apiKey
        model
        connected
        availableModels
      }
      todo {
        enrichment {
          enabled
          autoApplyThreshold
        }
        search {
          enabled
          semanticThreshold
          embeddingModel
          rrfK
        }
      }
    }
  }
`;

export const UPDATE_CONFIG = `
  mutation UpdateConfig($section: String!, $data: JSON!) {
    updateConfig(section: $section, data: $data) {
      success
      requiresRestart
    }
  }
`;

export const CONFIG_CHANGED_SUBSCRIPTION = `
  subscription ConfigChanged {
    configChanged {
      providers {
        name
        connected
      }
    }
  }
`;
