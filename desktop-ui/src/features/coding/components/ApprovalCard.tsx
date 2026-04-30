import { useEffect } from "react";
import type { ApprovalDecision } from "@/features/coding/hooks/useApprovalQueue";
import type { ConversationItem } from "@/types";

type ApprovalItem = Extract<ConversationItem, { kind: "approval" }>;

type Props = {
  item: ApprovalItem;
  onRespond: (requestId: string, decision: ApprovalDecision) => void;
};

export function ApprovalCard({ item, onRespond }: Props) {
  const pending = item.status === "pending";
  useEffect(() => {
    if (!pending) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "a") onRespond(item.requestId, { kind: "allow_once" });
      if (e.key === "s") onRespond(item.requestId, { kind: "allow_always" });
      if (e.key === "d") onRespond(item.requestId, { kind: "deny" });
      if (e.key === "r") {
        const src = window.prompt("Starlark rule source (Plan 4 will persist):", "");
        if (src != null) onRespond(item.requestId, { kind: "add_rule", starlark_source: src });
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [pending, item.requestId, onRespond]);

  if (!pending) {
    return (
      <div className="approval-card approval-card--decided">
        <span>
          {item.status === "approved-once" || item.status === "approved-always"
            ? "approved"
            : item.status === "denied"
              ? "denied"
              : item.status === "timed-out"
                ? "timed out"
                : "cancelled"}
          {" — "}
          {item.tool}: {summarizeArgs(item.args)}
        </span>
      </div>
    );
  }

  return (
    <div
      className="approval-card approval-card--pending"
      role="region"
      aria-label="Approval needed"
    >
      <header>Approval needed</header>
      <dl>
        <dt>Tool</dt>
        <dd>{item.tool}</dd>
        <dt>Args</dt>
        <dd className="approval-card__args">{summarizeArgs(item.args)}</dd>
        <dt>CWD</dt>
        <dd>{item.cwd}</dd>
        <dt>Sandbox</dt>
        <dd>{item.sandboxSummary}</dd>
        <dt>Layer</dt>
        <dd>
          {item.layer} — {item.layerReason}
        </dd>
        {item.mirrorHistory && (
          <>
            <dt>Mirror history</dt>
            <dd>
              {item.mirrorHistory.approvalCount} approvals · {item.mirrorHistory.denialCount}{" "}
              denials
            </dd>
          </>
        )}
      </dl>
      <div className="approval-card__buttons">
        <button type="button" onClick={() => onRespond(item.requestId, { kind: "allow_once" })}>
          Allow once (a)
        </button>
        <button type="button" onClick={() => onRespond(item.requestId, { kind: "allow_always" })}>
          Allow always (s)
        </button>
        <button type="button" onClick={() => onRespond(item.requestId, { kind: "deny" })}>
          Deny (d)
        </button>
        <button
          type="button"
          onClick={() => {
            const src = window.prompt("Starlark rule source (Plan 4):", "");
            if (src != null) onRespond(item.requestId, { kind: "add_rule", starlark_source: src });
          }}
        >
          Add rule… (r)
        </button>
      </div>
    </div>
  );
}

function summarizeArgs(a: Record<string, unknown>): string {
  if (typeof a.command === "string") return a.command;
  return JSON.stringify(a);
}
