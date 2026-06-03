import { useMemo, useState } from "react";

type PlanReadyFollowupMessageProps = {
  onAccept: () => void;
  onSubmitChanges: (changes: string) => void;
};

export function PlanReadyFollowupMessage({
  onAccept,
  onSubmitChanges,
}: PlanReadyFollowupMessageProps) {
  const [changes, setChanges] = useState("");
  const trimmed = useMemo(() => changes.trim(), [changes]);

  return (
    <div className="message items-start">
      <fieldset className="bubble w-[min(520px,72%)] max-w-full bg-surface-card-strong border border-border-stronger rounded-2xl p-2.5 px-3 flex flex-col gap-2" aria-label="Plan ready">
        <div className="flex justify-between items-baseline gap-2">
          <div className="text-ui-sm font-semibold text-text-strong">Plan ready</div>
        </div>
        <div className="grid gap-2">
          <section className="grid gap-1">
            <div className="text-ui-sm text-text-primary">
              Start building from this plan, or describe changes to the plan.
            </div>
            <textarea
              className="rounded-xl border border-border-subtle bg-surface-card-muted text-text-strong p-1.5 px-2 text-ui-sm leading-snug resize-y outline-none focus:outline focus:outline-2 focus:outline-[rgba(77,163,255,0.35)] focus:outline-offset-1"
              placeholder="Describe what you want to change in the plan..."
              value={changes}
              onChange={(event) => setChanges(event.target.value)}
              rows={3}
            />
          </section>
        </div>
        <div className="flex justify-end gap-2">
          <button
            type="button"
            className="bg-[#0b0f1a] text-white border border-border-strong"
            onClick={() => {
              if (!trimmed) {
                return;
              }
              onSubmitChanges(trimmed);
              setChanges("");
            }}
            disabled={!trimmed}
          >
            Send changes
          </button>
          <button type="button" className="primary" onClick={onAccept}>
            Implement this plan
          </button>
        </div>
      </fieldset>
    </div>
  );
}
