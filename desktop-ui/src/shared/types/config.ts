// ── MCP (Model Context Protocol) Settings ──────────────────

export interface McpServerConfig {
  name: string;
  transport: "stdio" | "http";
  enabled: boolean;
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  url?: string;
  headers?: Record<string, string>;
  oauthProvider?: string;
  oauthConnected: boolean;
}

export interface McpConfigResponse {
  enabled: boolean;
  servers: McpServerConfig[];
}

export type EmbeddedMcpState = "ready" | "disabled" | "invalid";

export interface EmbeddedMcpRejection {
  name: string;
  reason: string;
}

export interface EmbeddedMcpStatusResponse {
  state: EmbeddedMcpState | string;
  requested: string[];
  effective: string[];
  rejected: EmbeddedMcpRejection[];
}

export interface McpAddServerParams {
  name: string;
  transport: "stdio" | "http";
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  url?: string;
  headers?: Record<string, string>;
}

export interface McpToggleServerParams {
  name: string;
  enabled: boolean;
}

export interface RecommendedMcpServer {
  name: string;
  author: string;
  description: string;
  icon: string;
  transport: "stdio" | "http";
  command?: string;
  args?: string[];
  envKeys?: string[];
  url?: string;
  docsUrl?: string;
  oauthProvider?: string;
}

export interface OAuthStartParams {
  provider: string;
  serverName: string;
}
