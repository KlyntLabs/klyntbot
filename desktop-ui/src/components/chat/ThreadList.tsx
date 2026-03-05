import { FolderOpen, Globe, Plus, RotateCcw, Settings, Wallet } from "lucide-react";
import type { ChatThread } from "../../lib/types";
import { GroupHeader } from "./GroupHeader";
import { ThreadButton } from "./ThreadButton";

// Known feature prefixes -> display config
const FEATURE_GROUPS: Record<string, { label: string; icon: typeof Wallet }> = {
  finance: { label: "Finance", icon: Wallet },
};

function featurePrefix(entityKind?: string): string | null {
  if (!entityKind) return null;
  const dot = entityKind.indexOf(".");
  const prefix = dot > 0 ? entityKind.slice(0, dot) : entityKind;
  return FEATURE_GROUPS[prefix] ? prefix : null;
}

interface AreaGroup {
  areaId: string;
  areaName: string;
  projectGroups: Map<string, { projectName: string; threads: ChatThread[] }>;
  threads: ChatThread[];
}

interface ThreadListProps {
  threads: ChatThread[];
  grouped: {
    areas: AreaGroup[];
    features: Map<string, ChatThread[]>;
    general: ChatThread[];
  };
  selectedThread: string;
  expandedGroups: Set<string>;
  renaming: { sessionKey: string; value: string } | null;
  renameRef: React.RefObject<HTMLInputElement | null>;
  onSelectThread: (key: string) => void;
  onNewThread: () => void;
  onToggleGroup: (key: string) => void;
  onContextMenu: (e: React.MouseEvent, thread: ChatThread) => void;
  onRenameChange: (value: string) => void;
  onRenameConfirm: () => void;
  onRenameCancel: () => void;
}

export function ThreadList({
  threads,
  grouped,
  selectedThread,
  expandedGroups,
  renaming,
  renameRef,
  onSelectThread,
  onNewThread,
  onToggleGroup,
  onContextMenu,
  onRenameChange,
  onRenameConfirm,
  onRenameCancel,
}: ThreadListProps) {
  const renderThread = (thread: ChatThread) => (
    <ThreadButton
      key={thread.sessionKey}
      thread={thread}
      isActive={selectedThread === thread.sessionKey}
      isRenaming={renaming?.sessionKey === thread.sessionKey}
      renameValue={renaming?.value ?? ""}
      onSelect={onSelectThread}
      onContextMenu={onContextMenu}
      onRenameChange={onRenameChange}
      onRenameConfirm={onRenameConfirm}
      onRenameCancel={onRenameCancel}
      renameRef={renaming?.sessionKey === thread.sessionKey ? renameRef : undefined}
    />
  );

  return (
    <div className="w-[250px] glass-sidebar flex flex-col">
      {/* Quick Links */}
      <div className="px-4 py-3 space-y-1">
        <button
          type="button"
          onClick={onNewThread}
          className="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-white/[0.05] transition-all text-[12px] font-light text-muted hover:text-secondary"
        >
          <Plus className="w-[13px] h-[13px]" strokeWidth={1.5} />
          New thread
        </button>
        <button
          type="button"
          className="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-white/[0.05] transition-all text-[12px] font-light text-muted hover:text-secondary"
        >
          <RotateCcw className="w-[13px] h-[13px]" strokeWidth={1.5} />
          Automations
        </button>
        <button
          type="button"
          className="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-white/[0.05] transition-all text-[12px] font-light text-muted hover:text-secondary"
        >
          <Settings className="w-[13px] h-[13px]" strokeWidth={1.5} />
          Skills and Apps
        </button>
      </div>

      <div className="mx-4 glass-divider" />

      {/* Thread List */}
      <div className="flex-1 overflow-y-auto px-3 pb-3 pt-2">
        <div className="space-y-3">
          {/* PARA: Area groups */}
          {grouped.areas.map((area) => (
            <div key={area.areaId}>
              <GroupHeader
                groupKey={`area:${area.areaId}`}
                label={area.areaName}
                icon={FolderOpen}
                isExpanded={expandedGroups.has(`area:${area.areaId}`)}
                onToggle={onToggleGroup}
              />
              {expandedGroups.has(`area:${area.areaId}`) && (
                <div className="mt-1 ml-3 space-y-2">
                  {Array.from(area.projectGroups.entries()).map(([pid, pg]) => (
                    <div key={pid}>
                      <GroupHeader
                        groupKey={`proj:${pid}`}
                        label={pg.projectName}
                        icon={FolderOpen}
                        isExpanded={expandedGroups.has(`proj:${pid}`)}
                        onToggle={onToggleGroup}
                      />
                      {expandedGroups.has(`proj:${pid}`) && (
                        <div className="mt-1 ml-3 space-y-1">{pg.threads.map(renderThread)}</div>
                      )}
                    </div>
                  ))}
                  {area.threads.length > 0 && (
                    <div className="space-y-1">{area.threads.map(renderThread)}</div>
                  )}
                </div>
              )}
            </div>
          ))}

          {/* Feature groups */}
          {Array.from(grouped.features.entries()).map(([prefix, fThreads]) => {
            const cfg = FEATURE_GROUPS[prefix];
            return (
              <div key={`feat:${prefix}`}>
                <GroupHeader
                  groupKey={`feat:${prefix}`}
                  label={cfg.label}
                  icon={cfg.icon}
                  isExpanded={expandedGroups.has(`feat:${prefix}`)}
                  onToggle={onToggleGroup}
                />
                {expandedGroups.has(`feat:${prefix}`) && (
                  <div className="mt-1 ml-3 space-y-1">{fThreads.map(renderThread)}</div>
                )}
              </div>
            );
          })}

          {/* General threads */}
          {grouped.general.length > 0 && (
            <div>
              <GroupHeader
                groupKey="_general"
                label="General"
                icon={Globe}
                isExpanded={expandedGroups.has("_general")}
                onToggle={onToggleGroup}
              />
              {expandedGroups.has("_general") && (
                <div className="mt-1 ml-3 space-y-1">{grouped.general.map(renderThread)}</div>
              )}
            </div>
          )}

          {threads.length === 0 && (
            <div className="text-center py-8 text-muted text-[12px] font-light">
              No conversations yet
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export { type AreaGroup, FEATURE_GROUPS, featurePrefix };
