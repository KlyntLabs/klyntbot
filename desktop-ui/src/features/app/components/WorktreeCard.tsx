import type { MouseEvent } from "react";
import { cn } from "@/utils/cn";

import type { WorkspaceInfo } from "@/types";

type WorktreeCardProps = {
  worktree: WorkspaceInfo;
  isActive: boolean;
  isDeleting?: boolean;
  onSelectWorkspace: (id: string) => void;
  onShowWorktreeMenu: (event: MouseEvent, worktree: WorkspaceInfo) => void;
  onToggleWorkspaceCollapse: (workspaceId: string, collapsed: boolean) => void;
  onConnectWorkspace: (workspace: WorkspaceInfo) => void;
  children?: React.ReactNode;
};

export function WorktreeCard({
  worktree,
  isActive,
  isDeleting = false,
  onSelectWorkspace,
  onShowWorktreeMenu,
  onToggleWorkspaceCollapse,
  onConnectWorkspace,
  children,
}: WorktreeCardProps) {
  const worktreeCollapsed = worktree.settings.sidebarCollapsed;
  const worktreeBranch = worktree.worktree?.branch ?? "";
  const worktreeLabel = worktree.name?.trim() || worktreeBranch;
  const worktreeMeta = worktreeBranch && worktreeBranch !== worktreeLabel ? worktreeBranch : null;
  const contentCollapsedClass = worktreeCollapsed ? " collapsed" : "";

  return (
    <div className={isDeleting ? "rounded-lg border border-border-subtle bg-surface-card overflow-hidden opacity-60" : "rounded-lg border border-border-subtle bg-surface-card overflow-hidden"}>
      <button
        type="button"
        className={cn("flex items-center gap-2 px-3 py-2 w-full text-left bg-transparent border-none cursor-pointer", isActive && "bg-surface-active", isDeleting && "opacity-60 cursor-default")}
        disabled={isDeleting}
        onClick={() => {
          if (!isDeleting) {
            onSelectWorkspace(worktree.id);
          }
        }}
        onContextMenu={(event) => {
          if (!isDeleting) {
            onShowWorktreeMenu(event, worktree);
          }
        }}
        onKeyDown={(event) => {
          if (isDeleting) {
            return;
          }
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            onSelectWorkspace(worktree.id);
          }
        }}
      >
        <div className="min-w-0 flex-1">
          <div className="text-ui-sm font-semibold leading-tight text-text-strong whitespace-nowrap overflow-hidden text-ellipsis">{worktreeLabel}</div>
          {worktreeMeta && <div className="mt-1 text-ui-2xs leading-tight text-text-muted whitespace-nowrap overflow-hidden text-ellipsis">{worktreeMeta}</div>}
        </div>
        <div className="inline-flex items-center gap-1.5">
          {isDeleting ? (
            <div className="inline-flex items-center gap-1.5 text-ui-xs text-text-faint" role="status" aria-live="polite">
              <span className="w-2.5 h-2.5 rounded-full border-2 border-border-subtle border-t-text-strong animate-[spin_var(--duration-ui-spinner)_linear_infinite]" aria-hidden />
              <span className="tracking-wide">Deleting</span>
            </div>
          ) : (
            <>
              <button
                type="button"
                className={cn("border-none bg-transparent text-text-muted inline-flex items-center justify-center text-ui-sm leading-none px-0.5 [-webkit-app-region:no-drag] opacity-45 transition-opacity hover:text-text-strong hover:opacity-100 focus-visible:opacity-100", !worktreeCollapsed && "[&>span]:rotate-90")}
                onClick={(event) => {
                  event.stopPropagation();
                  onToggleWorkspaceCollapse(worktree.id, !worktreeCollapsed);
                }}
                data-tauri-drag-region="false"
                aria-label={worktreeCollapsed ? "Show agents" : "Hide agents"}
                aria-expanded={!worktreeCollapsed}
              >
                <span className="inline-block transition-transform duration-150">›</span>
              </button>
              {!worktree.connected && (
                <button
                  type="button"
                  className="connect"
                  title="Connect workspace context to the shared Codex server"
                  onClick={(event) => {
                    event.stopPropagation();
                    onConnectWorkspace(worktree);
                  }}
                >
                  connect
                </button>
              )}
            </>
          )}
        </div>
      </button>
      <div
        className={`worktree-card-content${contentCollapsedClass}`}
        aria-hidden={worktreeCollapsed}
        inert={worktreeCollapsed ? true : undefined}
      >
        <div className="worktree-card-content-inner">{children}</div>
      </div>
    </div>
  );
}
