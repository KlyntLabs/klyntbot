export type SetupStepId =
  | "welcome"
  | "provider"
  | "channels"
  | "areas"
  | "productivity"
  | "finance"
  | "mcp"
  | "complete";

export interface SetupStep {
  id: SetupStepId;
  path: string;
  label: string;
  required: boolean;
}

export interface SetupContext {
  next: () => void;
  back: () => void;
  skip: () => void;
}

export const SETUP_STEPS: SetupStep[] = [
  { id: "welcome", path: "/setup/welcome", label: "Welcome", required: true },
  { id: "provider", path: "/setup/provider", label: "Provider & Model", required: true },
  { id: "channels", path: "/setup/channels", label: "Channels", required: false },
  { id: "areas", path: "/setup/areas", label: "Areas", required: false },
  { id: "productivity", path: "/setup/productivity", label: "Productivity", required: false },
  { id: "finance", path: "/setup/finance", label: "Finance", required: false },
  { id: "mcp", path: "/setup/mcp", label: "MCP Servers", required: false },
  { id: "complete", path: "/setup/complete", label: "Complete", required: true },
];
