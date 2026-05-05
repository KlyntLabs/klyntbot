export type SlashPath = "agent" | "direct";

export type SlashLeaf = {
  kind: "leaf";
  path: SlashPath;
  command: string;
  description: string;
  category: SlashCategory;
  argHint?: string;
  tauriCommand?: string;
  requiresConfirmation?: boolean;
  agentTransform?: () => string;
};

export type SlashBranch = {
  kind: "branch";
  command: string;
  description: string;
  category: SlashCategory;
  children: Record<string, SlashNode>;
};

export type SlashNode = SlashLeaf | SlashBranch;

export type SlashCategory =
  | "mode"
  | "skills"
  | "status"
  | "sessions"
  | "permissions"
  | "recall"
  | "agent"
  | "help";

export type DispatchResult =
  | { kind: "passthrough"; text: string }
  | { kind: "render"; itemKind: string; item: unknown }
  | { kind: "error"; message: string };
