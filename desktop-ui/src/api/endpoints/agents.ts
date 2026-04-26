import { invoke } from "../client";


export type AgentSummary = {
  name: string;
  description: string | null;
  developerInstructions: string | null;
  configFile: string;
  resolvedPath: string;
  managedByApp: boolean;
  fileExists: boolean;
};

export type AgentsSettings = {
  configPath: string;
  multiAgentEnabled: boolean;
  maxThreads: number;
  maxDepth: number;
  agents: AgentSummary[];
};

export type SetAgentsCoreInput = {
  multiAgentEnabled: boolean;
  maxThreads: number;
  maxDepth: number;
};

export type CreateAgentInput = {
  name: string;
  description?: string | null;
  developerInstructions?: string | null;
  template?: "blank" | string | null;
  model?: string | null;
  reasoningEffort?: string | null;
};

export type UpdateAgentInput = {
  originalName: string;
  name: string;
  description?: string | null;
  developerInstructions?: string | null;
  renameManagedFile?: boolean;
};

export type DeleteAgentInput = {
  name: string;
  deleteManagedFile?: boolean;
};

export async function getAgentsSettings(): Promise<AgentsSettings> {
  return invoke<AgentsSettings>("get_agents_settings");
}

export async function setAgentsCoreSettings(
  input: SetAgentsCoreInput,
): Promise<AgentsSettings> {
  return invoke<AgentsSettings>("set_agents_core_settings", { input });
}

export async function createAgent(input: CreateAgentInput): Promise<AgentsSettings> {
  return invoke<AgentsSettings>("create_agent", { input });
}

export async function updateAgent(input: UpdateAgentInput): Promise<AgentsSettings> {
  return invoke<AgentsSettings>("update_agent", { input });
}

export async function deleteAgent(input: DeleteAgentInput): Promise<AgentsSettings> {
  return invoke<AgentsSettings>("delete_agent", { input });
}

export async function readAgentConfigToml(agentName: string): Promise<string> {
  return invoke<string>("read_agent_config_toml", { agentName });
}

export async function writeAgentConfigToml(
  agentName: string,
  content: string,
): Promise<void> {
  return invoke("write_agent_config_toml", { agentName, content });
}

export type GeneratedAgentConfiguration = {
  description: string;
  developerInstructions: string;
};

export async function generateAgentDescription(
  workspaceId: string,
  description: string,
): Promise<GeneratedAgentConfiguration> {
  return invoke("generate_agent_description", { workspaceId, description });
}
