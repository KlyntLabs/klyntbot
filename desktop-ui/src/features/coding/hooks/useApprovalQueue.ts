import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect } from "react";
import { invoke } from "@/api/client";
import { chatStreamStore } from "@/features/chat/store/chatStreamStore";
import type { ConversationItem } from "@/types";

type ApprovalPayload = {
  request_id: string;
  tool: string;
  args: Record<string, unknown>;
  cwd: string;
  sandbox_summary: string;
  layer: "privacy" | "layer1_declarative" | "layer2_starlark" | "layer3_mirror" | "default_mode";
  layer_reason: string;
  mirror_history?: { approval_count: number; denial_count: number };
  requires_user_input: boolean;
};

type ResolvedPayload = {
  request_id: string;
  decided_by: "user" | "auto_allow" | "auto_deny" | "timeout" | "cancelled";
  decision_reason: string;
};

export type ApprovalDecision =
  | { kind: "allow_once" }
  | { kind: "allow_always"; rule?: string }
  | { kind: "deny" }
  | { kind: "add_rule"; starlark_source: string };

type ApprovalItem = Extract<ConversationItem, { kind: "approval" }>;

function toItem(payload: ApprovalPayload): ApprovalItem {
  return {
    id: `approval-${payload.request_id}`,
    kind: "approval",
    requestId: payload.request_id,
    tool: payload.tool,
    args: payload.args,
    cwd: payload.cwd,
    sandboxSummary: payload.sandbox_summary,
    layer: payload.layer,
    layerReason: payload.layer_reason,
    mirrorHistory: payload.mirror_history
      ? {
          approvalCount: payload.mirror_history.approval_count,
          denialCount: payload.mirror_history.denial_count,
        }
      : undefined,
    status: "pending",
  };
}

function mapStatus(d: ResolvedPayload["decided_by"]): ApprovalItem["status"] {
  switch (d) {
    case "user":
      return "approved-once";
    case "auto_allow":
      return "approved-once";
    case "auto_deny":
      return "denied";
    case "timeout":
      return "timed-out";
    case "cancelled":
      return "cancelled";
  }
}

export function useApprovalQueue(sessionKey: string) {
  useEffect(() => {
    if (!sessionKey) return;
    const unlistens: Array<() => void> = [];
    let cancelled = false;

    (async () => {
      try {
        const un = await listen<ApprovalPayload>("agent:approval_requested", (e) => {
          if (!e.payload.requires_user_input) return;
          chatStreamStore.upsertApproval(sessionKey, toItem(e.payload));
        });
        if (cancelled) {
          un();
        } else {
          unlistens.push(un);
        }
      } catch {}

      try {
        const un = await listen<ResolvedPayload>("agent:approval_resolved", (e) => {
          chatStreamStore.resolveApproval(
            sessionKey,
            e.payload.request_id,
            mapStatus(e.payload.decided_by),
            e.payload.decided_by,
          );
        });
        if (cancelled) {
          un();
        } else {
          unlistens.push(un);
        }
      } catch {}
    })();

    return () => {
      cancelled = true;
      for (const f of unlistens) {
        f();
      }
    };
  }, [sessionKey]);

  const respond = useCallback(
    async (requestId: string, decision: ApprovalDecision) => {
      await invoke("chat_respond_approval", { sessionKey, requestId, decision });
    },
    [sessionKey],
  );

  return { respond };
}
