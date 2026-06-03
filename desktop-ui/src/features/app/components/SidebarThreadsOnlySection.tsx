import type { ThreadStatusById } from "@utils/threadStatus";
import Plus from "lucide-react/dist/esm/icons/plus";
import type { MouseEvent, MutableRefObject } from "react";
import { createPortal } from "react-dom";
import {
  PopoverMenuItem,
  PopoverSurface,
} from "@/features/design-system/components/popover/PopoverPrimitives";
import type { ThreadSummary, WorkspaceInfo } from "@/types";
import { PinnedThreadList } from "./PinnedThreadList";
import type { SidebarOverlayMenuAnchor, ThreadBucket } from "./sidebarTypes";

type SidebarThreadsOnlySectionProps = {
  threadBuckets: ThreadBucket[];
  activeWorkspaceId: string | null;
  activeThreadId: string | null;
  threadStatusById: ThreadStatusById;
  pendingUserInputKeys?: Set<string>;
  getThreadTime: (thread: ThreadSummary) => string | null;
  getThreadArgsBadge?: (workspaceId: string, threadId: string) => string | null;
  isThreadPinned: (workspaceId: string, threadId: string) => boolean;
  onSelectThread: (workspaceId: string, threadId: string) => void;
  onShowThreadMenu: (
    event: MouseEvent,
    workspaceId: string,
    threadId: string,
    canPin: boolean,
  ) => void;
  getWorkspaceLabel: (workspaceId: string) => string | null;
  addMenuOpen: boolean;
  addMenuAnchor: SidebarOverlayMenuAnchor | null;
  addMenuRef: MutableRefObject<HTMLDivElement | null>;
  projectOptionsForNewThread: WorkspaceInfo[];
  onToggleAddMenu: (event: MouseEvent<HTMLButtonElement>) => void;
  onCreateThreadInProject: (workspace: WorkspaceInfo) => void;
};

export function SidebarThreadsOnlySection({
  threadBuckets,
  activeWorkspaceId,
  activeThreadId,
  threadStatusById,
  pendingUserInputKeys,
  getThreadTime,
  getThreadArgsBadge,
  isThreadPinned,
  onSelectThread,
  onShowThreadMenu,
  getWorkspaceLabel,
  addMenuOpen,
  addMenuAnchor,
  addMenuRef,
  projectOptionsForNewThread,
  onToggleAddMenu,
  onCreateThreadInProject,
}: SidebarThreadsOnlySectionProps) {
  return (
    <div className="flex flex-col">
      <div className="flex items-center justify-between gap-2 px-1 pb-1 pt-0">
        <div className="text-ui-sm font-semibold tracking-wide text-text-strong">Recent conversations</div>
        <button
          type="button"
          className="ghost w-6 h-6 rounded-full border border-border-stronger bg-cm-surface-panel-loud text-text-muted inline-flex items-center justify-center p-0 [-webkit-app-region:no-drag] opacity-60 hover:opacity-100 hover:text-text-strong hover:bg-surface-card-strong"
          onClick={onToggleAddMenu}
          data-tauri-drag-region="false"
          aria-label="New thread in project"
          title="New thread in project"
          aria-expanded={addMenuOpen}
          disabled={projectOptionsForNewThread.length === 0}
        >
          <Plus aria-hidden />
        </button>
      </div>
      {threadBuckets.map((bucket) => (
        <div key={bucket.id} className="flex flex-col gap-2.5">
          <div className="flex items-center justify-between gap-2">
            <div className="text-ui-xs uppercase tracking-[0.08em] text-text-faint">{bucket.label}</div>
          </div>
          <PinnedThreadList
            rows={bucket.rows}
            activeWorkspaceId={activeWorkspaceId}
            activeThreadId={activeThreadId}
            threadStatusById={threadStatusById}
            pendingUserInputKeys={pendingUserInputKeys}
            getThreadTime={getThreadTime}
            getThreadArgsBadge={getThreadArgsBadge}
            isThreadPinned={isThreadPinned}
            onSelectThread={onSelectThread}
            onShowThreadMenu={onShowThreadMenu}
            getWorkspaceLabel={getWorkspaceLabel}
          />
        </div>
      ))}
      {addMenuAnchor &&
        createPortal(
          <PopoverSurface
            className="fixed isolate rounded-xl p-1.5 flex flex-col gap-1 min-w-[160px] z-[9999] max-h-[320px] overflow-y-auto"
            ref={addMenuRef}
            style={{
              top: addMenuAnchor.top,
              left: addMenuAnchor.left,
              width: addMenuAnchor.width,
            }}
          >
            {projectOptionsForNewThread.map((workspace) => (
              <PopoverMenuItem
                key={workspace.id}
                className="border-none bg-transparent text-text-strong text-ui-sm text-left px-2 py-1.5 rounded-md cursor-pointer hover:bg-surface-hover"
                onClick={(event) => {
                  event.stopPropagation();
                  onCreateThreadInProject(workspace);
                }}
                icon={<Plus aria-hidden />}
              >
                {workspace.name}
              </PopoverMenuItem>
            ))}
          </PopoverSurface>,
          document.body,
        )}
    </div>
  );
}
