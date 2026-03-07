import { useMemo, useState } from "react";
import { useWorkflows } from "../../hooks/useWorkflows";

interface WorkflowPickerProps {
  currentWorkflowId: string | null;
  onSelect: (workflowId: string | null) => void;
}

export function WorkflowPicker({ currentWorkflowId, onSelect }: WorkflowPickerProps) {
  const { data: workflows } = useWorkflows();
  const [isOpen, setIsOpen] = useState(false);

  const currentWorkflow = useMemo(
    () =>
      workflows.find((wf) => wf.id === currentWorkflowId) ??
      workflows.find((wf) => wf.isGlobalDefault),
    [workflows, currentWorkflowId],
  );

  const nonTemplates = useMemo(() => workflows.filter((wf) => !wf.isTemplate), [workflows]);

  const templates = useMemo(() => workflows.filter((wf) => wf.isTemplate), [workflows]);

  return (
    <div className="relative">
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className="flex items-center gap-2 px-3 py-1.5 rounded-md bg-white/[0.06] hover:bg-white/[0.08] transition-colors text-[12px] font-light text-muted"
      >
        <span>{currentWorkflow?.name ?? "Default"}</span>
        <svg className="w-3 h-3 text-dim" viewBox="0 0 12 12" fill="none" aria-hidden="true">
          <path d="M3 5L6 8L9 5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        </svg>
      </button>

      {isOpen && (
        <div className="absolute top-full mt-1 left-0 z-50 min-w-[200px] rounded-lg border border-white/[0.06] bg-surface-elevated shadow-lg py-1">
          {nonTemplates.map((wf) => (
            <button
              key={wf.id}
              type="button"
              onClick={() => {
                onSelect(wf.isGlobalDefault ? null : wf.id);
                setIsOpen(false);
              }}
              className={`w-full text-left px-3 py-2 text-[12px] font-light transition-colors hover:bg-white/[0.06] flex items-center justify-between ${
                wf.id === (currentWorkflowId ?? currentWorkflow?.id) ? "text-brand" : "text-muted"
              }`}
            >
              <span>{wf.name}</span>
              <span className="text-[10px] text-dim">{wf.labels.length} statuses</span>
            </button>
          ))}

          {templates.length > 0 && (
            <>
              <div className="border-t border-white/[0.06] my-1" />
              <div className="px-3 py-1">
                <span className="text-[10px] text-dim uppercase tracking-wider">Templates</span>
              </div>
              {templates.map((wf) => (
                <button
                  key={wf.id}
                  type="button"
                  onClick={() => {
                    onSelect(wf.id);
                    setIsOpen(false);
                  }}
                  className="w-full text-left px-3 py-2 text-[12px] font-light text-muted hover:bg-white/[0.06] flex items-center justify-between"
                >
                  <span>{wf.name}</span>
                  <span className="text-[10px] text-dim">{wf.labels.length} statuses</span>
                </button>
              ))}
            </>
          )}
        </div>
      )}
    </div>
  );
}
