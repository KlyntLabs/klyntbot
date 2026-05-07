import { useEffect, useState } from "react";
import type { ApprovalDecision } from "@/features/coding/hooks/useApprovalQueue";
import type { ConversationItem } from "@/types";
import { PreviewRenderer } from "./preview/PreviewRenderer";
import { SmartAllowAlwaysButton } from "./SmartAllowAlwaysButton";
import { StarlarkRuleEditor } from "./StarlarkRuleEditor";

type ApprovalItem = Extract<ConversationItem, { kind: "approval" }>;

type Props = {
  item: ApprovalItem;
  onRespond: (requestId: string, decision: ApprovalDecision) => void;
};

export function ApprovalCard({ item, onRespond }: Props) {
  const pending = item.status === "pending";
  const [editorOpen, setEditorOpen] = useState(false);
  useEffect(() => {
    if (!pending) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "a") onRespond(item.requestId, { kind: "allow_once" });
      if (e.key === "s") onRespond(item.requestId, { kind: "allow_always" });
      if (e.key === "d") onRespond(item.requestId, { kind: "deny" });
      if (e.key === "r") setEditorOpen(true);
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [pending, item.requestId, onRespond]);

  if (!pending) {
    return (
      <div className="approval-card approval-card--decided">
        <span>
          {statusLabel(item.status)}
          {" — "}
          {item.tool}: {summarizeArgs(item.args)}
        </span>
      </div>
    );
  }

  return (
    <section className="approval-card approval-card--pending" aria-label="Approval needed">
      <header>Approval needed</header>
      <dl>
        <dt>Tool</dt>
        <dd>{item.tool}</dd>
        <dt>Preview</dt>
        <dd className="approval-card__preview">
          {item.preview ? (
            <PreviewRenderer preview={item.preview} />
          ) : (
            summarizeArgs(item.args)
          )}
        </dd>
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
      {item.layerDecisions && (
        <details className="approval-card__why">
          <summary>Why am I being asked?</summary>
          <dl className="approval-card__layer-audit">
            <dt>Privacy</dt>
            <dd>{item.layerDecisions.privacy.reason}</dd>
            <dt>Layer 1 (declarative)</dt>
            <dd>{item.layerDecisions.layer1.reason}</dd>
            <dt>Layer 2 (Starlark)</dt>
            <dd>{item.layerDecisions.layer2.reason}</dd>
            <dt>Layer 3 (Mirror)</dt>
            <dd>{item.layerDecisions.layer3.reason}</dd>
          </dl>
        </details>
      )}
      <div className="approval-card__buttons">
        <button type="button" onClick={() => onRespond(item.requestId, { kind: "allow_once" })}>
          Allow once (a)
        </button>
        <SmartAllowAlwaysButton
          requestId={item.requestId}
          suggestedGrant={item.suggestedGrant ?? null}
          onRespond={onRespond}
          onOpenStarlarkEditor={() => setEditorOpen(true)}
        />
        <button type="button" onClick={() => onRespond(item.requestId, { kind: "deny" })}>
          Deny (d)
        </button>
        <button type="button" onClick={() => setEditorOpen(true)}>
          Add rule... (r)
        </button>
      </div>
      {editorOpen && (
        <StarlarkRuleEditor
          requestId={item.requestId}
          onCommit={(_path) => {
            onRespond(item.requestId, { kind: "add_rule", starlark_source: "" });
            setEditorOpen(false);
          }}
          onCancel={() => setEditorOpen(false)}
        />
      )}
    </section>
  );
}

function statusLabel(status: ApprovalItem["status"]): string {
  switch (status) {
    case "approved-once":
    case "approved-always":
      return "approved";
    case "denied":
      return "denied";
    case "timed-out":
      return "timed out";
    default:
      return "cancelled";
  }
}

function summarizeArgs(a: Record<string, unknown>): string {
  if (typeof a.command === "string") return a.command;
  return JSON.stringify(a);
}
