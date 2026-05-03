export type MessagePart =
  | { kind: "text"; text: string }
  | { kind: "tool_call"; call_id: string; name: string; args: unknown }
  | { kind: "tool_result"; call_id: string; output: ToolOutput; is_error: boolean }
  | { kind: "reasoning"; text: string; redacted: boolean }
  | { kind: "file_change"; path: string; before: string | null; after: string; diff_unified: string; applied: boolean }
  | { kind: "command_execution"; command: string[]; cwd: string; exit_code: number | null; stdout: string; stderr: string }
  | { kind: "finish"; reason: FinishReason };

export type ToolOutput = {
  text: string;
  mime: string | null;
  truncated: boolean;
};

export type FinishReason =
  | { kind: "completed" }
  | { kind: "tool_calls_exhausted" }
  | { kind: "cancelled" }
  | { kind: "permission_denied"; reason: string }
  | { kind: "sandbox_violation"; reason: string }
  | { kind: "cost_ceiling_reached"; spend_usd: number; ceiling_usd: number }
  | { kind: "error"; code: string; message: string; retryable: boolean };
