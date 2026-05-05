import { invoke } from "../client";

export type ApprovalDecisionDto =
  | { kind: "accept" }
  | { kind: "decline" }
  | { kind: "accept_for_session" }
  | {
      kind: "accept_with_execpolicy_amendment";
      execpolicy_amendment: {
        command_pattern: string[];
        starlark_source: string | null;
      };
    }
  | { kind: "cancel" };

export async function approvalRespond(
  approvalId: string,
  decision: ApprovalDecisionDto,
): Promise<void> {
  return invoke<void>("approval_respond", { approvalId, decision });
}
