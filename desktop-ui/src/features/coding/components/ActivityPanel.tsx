import { PanelShell } from "@/features/layout/components/PanelShell";
import type { PanelTabId } from "@/features/layout/components/PanelTabs";
import { JobsPanel } from "./JobsPanel";
import { TodoPanel } from "./TodoPanel";

type ActivityPanelProps = {
  threadId: string | null;
  filePanelMode: PanelTabId;
  onFilePanelModeChange: (mode: PanelTabId) => void;
};

export function ActivityPanel({
  threadId,
  filePanelMode,
  onFilePanelModeChange,
}: ActivityPanelProps) {
  return (
    <PanelShell
      filePanelMode={filePanelMode}
      onFilePanelModeChange={onFilePanelModeChange}
      headerClassName="activity-panel-header"
    >
      <div className="coding-activity-panel">
        {threadId ? (
          <>
            <TodoPanel threadId={threadId} />
            <JobsPanel threadId={threadId} />
          </>
        ) : (
          <p className="coding-activity-panel__empty">
            Open a coding chat to see todos and background jobs here.
          </p>
        )}
      </div>
    </PanelShell>
  );
}
