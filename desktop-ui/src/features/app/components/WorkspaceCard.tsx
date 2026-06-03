import type { MouseEvent } from "react";
import { cn } from "@/utils/cn";
import type { WorkspaceInfo } from "@/types";

type WorkspaceCardProps = {
  workspace: WorkspaceInfo;
  workspaceName?: React.ReactNode;
  summary?: string | null;
  isActive: boolean;
  isCollapsed: boolean;
  addMenuOpen: boolean;
  addMenuWidth: number;
  onSelectWorkspace: (id: string) => void;
  onShowWorkspaceMenu: (event: MouseEvent, workspaceId: string) => void;
  onToggleWorkspaceCollapse: (workspaceId: string, collapsed: boolean) => void;
  onConnectWorkspace: (workspace: WorkspaceInfo) => void;
  onToggleAddMenu: (
    anchor: {
      workspaceId: string;
      top: number;
      left: number;
      width: number;
    } | null,
  ) => void;
  children?: React.ReactNode;
};

export function WorkspaceCard({
  workspace,
  workspaceName,
  summary = null,
  isActive,
  isCollapsed,
  addMenuOpen,
  addMenuWidth,
  onSelectWorkspace,
  onShowWorkspaceMenu,
  onToggleWorkspaceCollapse,
  onConnectWorkspace,
  onToggleAddMenu,
  children,
}: WorkspaceCardProps) {
  return (
    <div className="workspace-card flex flex-col gap-2 overflow-x-hidden">
      <button
        type="button"
        className={cn("workspace-row", isActive && "active")}
        onClick={() => onSelectWorkspace(workspace.id)}
        onContextMenu={(event) => onShowWorkspaceMenu(event, workspace.id)}
        onKeyDown={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            onSelectWorkspace(workspace.id);
          }
        }}
      >
        <div className="workspace-copy min-w-0 pr-[6px]">
          <div className="workspace-name-row flex items-center gap-[6px] min-w-0">
            <div className="workspace-title flex items-start gap-[6px] min-w-0">
              <span className="workspace-name font-semibold text-ui-md leading-[1.15] min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">{workspaceName ?? workspace.name}</span>
              <button
                type="button"
                className={`workspace-toggle ${isCollapsed ? "" : "expanded"}`}
                onClick={(event) => {
                  event.stopPropagation();
                  onToggleWorkspaceCollapse(workspace.id, !isCollapsed);
                }}
                data-tauri-drag-region="false"
                aria-label={isCollapsed ? "Show agents" : "Hide agents"}
                aria-expanded={!isCollapsed}
              >
                <span className="workspace-toggle-icon inline-block transition-transform duration-[150ms] ease-out">›</span>
              </button>
            </div>
          </div>
          {summary && <div className="workspace-summary mt-[5px] text-ui-2xs leading-tight text-text-muted whitespace-nowrap overflow-hidden text-ellipsis">{summary}</div>}
        </div>
        <div className="workspace-actions inline-flex flex-col items-end justify-start gap-2 shrink-0 pt-0.5">
          <button
            type="button"
            className="ghost workspace-add w-[22px] h-[22px] rounded-full border border-border-stronger bg-[var(--cm-surface-panel-loud)] text-text-muted inline-flex items-center justify-center text-ui-sm leading-none [webkit-app-region:no-drag] shrink-0 opacity-[0.46] transition-opacity duration-[150ms] ease-out"
            onClick={(event) => {
              event.stopPropagation();
              const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
              const left = Math.min(Math.max(rect.left, 12), window.innerWidth - addMenuWidth - 12);
              const top = rect.bottom + 8;
              onToggleAddMenu(
                addMenuOpen
                  ? null
                  : {
                      workspaceId: workspace.id,
                      top,
                      left,
                      width: addMenuWidth,
                    },
              );
            }}
            data-tauri-drag-region="false"
            aria-label="Add agent options"
            aria-expanded={addMenuOpen}
          >
            +
          </button>
          {!workspace.connected && (
            <button
              type="button"
              className="connect text-ui-sm text-text-muted self-center shrink-0 py-0.5 px-2 rounded-full border border-border-quiet [webkit-app-region:no-drag]"
              title="Connect workspace context to the shared Codex server"
              onClick={(event) => {
                event.stopPropagation();
                onConnectWorkspace(workspace);
              }}
            >
              connect
            </button>
          )}
        </div>
      </button>
      <div
        className={cn(
          "workspace-card-content grid grid-rows-[1fr] opacity-100 translate-y-0 transition-[grid-template-rows,opacity,transform] duration-[200ms] ease-out",
          isCollapsed && "grid-rows-[0fr] opacity-0 -translate-y-1 pointer-events-none",
        )}
        aria-hidden={isCollapsed}
        inert={isCollapsed ? true : undefined}
      >
        <div className="workspace-card-content-inner flex flex-col gap-[10px] pt-[3px] overflow-hidden min-h-0">{children}</div>
      </div>
    </div>
  );
}
