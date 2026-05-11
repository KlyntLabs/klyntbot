import { useState } from "react";
import type { ApprovalDecision } from "@/features/coding/hooks/useApprovalQueue";
import { PatternPicker } from "./PatternPicker";
import type { SuggestedGrant } from "./preview/types";

type Props = {
  requestId: string;
  suggestedGrant: SuggestedGrant | null;
  onRespond: (requestId: string, decision: ApprovalDecision) => void;
  onOpenStarlarkEditor: () => void;
};

export function SmartAllowAlwaysButton({
  requestId,
  suggestedGrant,
  onRespond,
  onOpenStarlarkEditor,
}: Props) {
  const [pickerOpen, setPickerOpen] = useState(false);

  if (!suggestedGrant) {
    return (
      <button type="button" onClick={() => onRespond(requestId, { kind: "allow_always" })}>
        Allow always (s)
      </button>
    );
  }

  const commit = (rule: string) => {
    onRespond(requestId, { kind: "allow_always", rule });
    setPickerOpen(false);
  };

  return (
    <div className="approval-card__smart-allow-always">
      <div className="approval-card__split-button">
        <button
          type="button"
          className="approval-card__split-primary"
          onClick={() => commit(suggestedGrant.pattern)}
          title={suggestedGrant.reason}
        >
          Allow always: <strong>{suggestedGrant.pattern}</strong>
        </button>
        <button
          type="button"
          className="approval-card__split-caret"
          aria-label="Refine pattern"
          onClick={() => setPickerOpen((o) => !o)}
        >
          ▾
        </button>
      </div>
      {pickerOpen && (
        <PatternPicker
          suggested={suggestedGrant}
          onCommit={commit}
          onCustom={onOpenStarlarkEditor}
        />
      )}
    </div>
  );
}
