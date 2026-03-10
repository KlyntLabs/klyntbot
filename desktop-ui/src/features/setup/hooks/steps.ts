import type { MutableRefObject } from "react";

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
  /** Ref for forward handler. Receives isSkip flag. Return true → layout navigates to next main step. */
  forwardRef: MutableRefObject<((isSkip: boolean) => Promise<boolean>) | null>;
  /** Ref for back handler. Return true → layout navigates to previous main step. */
  backRef: MutableRefObject<(() => boolean) | null>;
  /** Mark step as having user modifications (toggles Skip → Continue for optional steps). */
  setDirty: (dirty: boolean) => void;
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
