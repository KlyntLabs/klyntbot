import { getApprovalCommandInfo } from "@utils/approvalRules";
import { useEffect, useMemo } from "react";
import {
  ToastActions,
  ToastBody,
  ToastCard,
  ToastError,
  ToastHeader,
  ToastTitle,
  ToastViewport,
} from "@/features/design-system/components/toast/ToastPrimitives";
import type { ApprovalRequest, WorkspaceInfo } from "@/types";
import "./ApprovalToasts.css";

type ApprovalToastsProps = {
  approvals: ApprovalRequest[];
  workspaces: WorkspaceInfo[];
  onDecision: (request: ApprovalRequest, decision: "accept" | "decline") => void;
  onRemember?: (request: ApprovalRequest, command: string[]) => void;
};

export function ApprovalToasts({
  approvals,
  workspaces,
  onDecision,
  onRemember,
}: ApprovalToastsProps) {
  const workspaceLabels = useMemo(
    () => new Map(workspaces.map((workspace) => [workspace.id, workspace.name])),
    [workspaces],
  );

  const primaryRequest = approvals[approvals.length - 1];

  useEffect(() => {
    if (!primaryRequest) {
      return;
    }

    const handler = (event: KeyboardEvent) => {
      if (event.key !== "Enter") {
        return;
      }
      const active = document.activeElement;
      if (
        active instanceof HTMLElement &&
        (active.isContentEditable ||
          active.tagName === "INPUT" ||
          active.tagName === "TEXTAREA" ||
          active.tagName === "SELECT")
      ) {
        return;
      }
      event.preventDefault();
      onDecision(primaryRequest, "accept");
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onDecision, primaryRequest]);

  if (!approvals.length) {
    return null;
  }

  const formatLabel = (value: string) =>
    value
      .replace(/([a-z])([A-Z])/g, "$1 $2")
      .replace(/_/g, " ")
      .trim();

  const methodLabel = (method: string) => {
    const trimmed = method.replace(/^codex\/requestApproval\/?/, "");
    return trimmed || method;
  };

  const renderParamValue = (value: unknown) => {
    if (value === null || value === undefined) {
      return { text: "None", isCode: false };
    }
    if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
      return { text: String(value), isCode: false };
    }
    if (Array.isArray(value)) {
      if (value.every((entry) => ["string", "number", "boolean"].includes(typeof entry))) {
        return { text: value.map(String).join(", "), isCode: false };
      }
      return { text: JSON.stringify(value, null, 2), isCode: true };
    }
    return { text: JSON.stringify(value, null, 2), isCode: true };
  };

  return (
    <ToastViewport className="fixed top-1/2 left-1/2 w-[min(460px,calc(100vw-48px))] max-h-[min(520px,calc(100vh-140px))] overflow-auto z-5 pointer-events-auto [-webkit-app-region:no-drag] -translate-x-1/2 -translate-y-1/2" role="region" ariaLive="assertive">
      {approvals.map((request) => {
        const workspaceName = workspaceLabels.get(request.workspace_id);
        const params = request.params ?? {};
        const commandInfo = getApprovalCommandInfo(params);
        const entries = Object.entries(params);
        return (
          <ToastCard
            key={`${request.workspace_id}-${request.request_id}`}
            className="[--ds-toast-enter-duration:0.2s]"
            role="alert"
          >
            <ToastHeader className="mb-1.5">
              <ToastTitle className="text-ui-sm tracking-[0.08em] uppercase">Approval needed</ToastTitle>
              {workspaceName ? (
                <div className="text-ui-sm text-text-faint">{workspaceName}</div>
              ) : null}
            </ToastHeader>
            <div className="text-ui-sm font-semibold mb-2 break-words">{methodLabel(request.method)}</div>
            <div className="grid gap-2 mb-2.5">
              {entries.length ? (
                entries.map(([key, value]) => {
                  const rendered = renderParamValue(value);
                  return (
                    <div key={key} className="grid gap-1">
                      <div className="text-ui-xs text-text-faint uppercase tracking-[0.06em]">{formatLabel(key)}</div>
                      {rendered.isCode ? (
                        <ToastError className="max-h-40">
                          {rendered.text}
                        </ToastError>
                      ) : (
                        <ToastBody className="text-ui-sm break-words">
                          {rendered.text}
                        </ToastBody>
                      )}
                    </div>
                  );
                })
              ) : (
                <div className="approval-toast-detail approval-toast-detail-empty">
                  No extra details.
                </div>
              )}
            </div>
            <ToastActions className="approval-toast-actions">
              <button
                type="button"
                className="secondary"
                onClick={() => onDecision(request, "decline")}
              >
                Decline
              </button>
              {commandInfo && onRemember ? (
                <button
                  type="button"
                  className="ghost whitespace-nowrap"
                  onClick={() => onRemember(request, commandInfo.tokens)}
                  title={`Allow commands that start with ${commandInfo.preview}`}
                >
                  Always allow
                </button>
              ) : null}
              <button
                type="button"
                className="primary"
                onClick={() => onDecision(request, "accept")}
              >
                Approve (Enter)
              </button>
            </ToastActions>
          </ToastCard>
        );
      })}
    </ToastViewport>
  );
}
