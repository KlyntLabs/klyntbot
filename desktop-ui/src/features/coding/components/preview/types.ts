/**
 * Approval preview types — mirror the Rust `ApprovalPreview` enum.
 * These will be auto-generated via specta once `cargo tauri dev` regenerates bindings.
 * Manual definitions for now.
 */

export type ApprovalPreview =
  | {
      kind: "diff";
      path: string;
      unified_diff: string;
      lines_added: number;
      lines_removed: number;
      is_new_file: boolean;
      is_truncated: boolean;
    }
  | {
      kind: "command";
      command: string;
      cwd: string;
      is_dangerous: boolean;
      risk_hits: string[];
    }
  | {
      kind: "url";
      method: string;
      url: string;
      headers: [string, string][];
      body_preview: string | null;
    }
  | {
      kind: "mcp";
      server: string;
      tool: string;
      args: Record<string, unknown>;
      schema: Record<string, unknown> | null;
    }
  | {
      kind: "generic";
      args: Record<string, unknown>;
    };

export type GrantScope =
  | { kind: "exact_tool_path"; tool: string; path: string }
  | { kind: "tool_folder"; tool: string; folder: string }
  | { kind: "tool_glob"; tool: string; glob: string }
  | { kind: "custom"; starlark_source: string };

export interface SuggestedGrant {
  pattern: string;
  scope: GrantScope;
  reason: string;
}
