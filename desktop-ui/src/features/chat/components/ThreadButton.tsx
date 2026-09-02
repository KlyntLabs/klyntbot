import { formatRelativeTime } from "@shared/lib/dates";
import type { ChatThread } from "@shared/types";
import { Check, MessageSquare, Users, X } from "lucide-react";

interface ThreadButtonProps {
  thread: ChatThread;
  isActive: boolean;
  isRenaming: boolean;
  renameValue: string;
  onSelect: (key: string) => void;
  onContextMenu: (e: React.MouseEvent, thread: ChatThread) => void;
  onRenameChange: (value: string) => void;
  onRenameConfirm: () => void;
  onRenameCancel: () => void;
  renameRef?: React.Ref<HTMLInputElement>;
}

export function ThreadButton({
  thread,
  isActive,
  isRenaming,
  renameValue,
  onSelect,
  onContextMenu,
  onRenameChange,
  onRenameConfirm,
  onRenameCancel,
  renameRef,
}: ThreadButtonProps) {
  if (isRenaming) {
    return (
      <div className="flex items-center gap-1 px-2 py-1">
        <input
          ref={renameRef}
          value={renameValue}
          onChange={(e) => onRenameChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") onRenameConfirm();
            if (e.key === "Escape") onRenameCancel();
          }}
          className="flex-1 min-w-0 bg-control-hover text-fg text-ui-sm font-light px-2 py-1 rounded border border-separator"
        />
        <button
          type="button"
          onClick={onRenameConfirm}
          aria-label="Confirm rename"
          className="text-status-success hover:text-status-success/80 shrink-0"
        >
          <Check className="size-3.5" strokeWidth={2} />
        </button>
        <button
          type="button"
          onClick={onRenameCancel}
          aria-label="Cancel rename"
          className="text-fg-secondary hover:text-fg shrink-0"
        >
          <X className="size-3.5" strokeWidth={2} />
        </button>
      </div>
    );
  }

  return (
    <button
      type="button"
      onClick={() => onSelect(thread.sessionKey)}
      onContextMenu={(e) => onContextMenu(e, thread)}
      className={`w-full flex items-center gap-2 px-3 py-1.5 rounded-lg transition-colors text-ui-sm font-light ${
        isActive
          ? "bg-control-hover text-fg"
          : "text-fg-secondary hover:bg-control-hover hover:text-fg"
      }`}
    >
      {thread.squadId ? (
        <Users className="size-3 shrink-0 text-purple-400" strokeWidth={1.5} />
      ) : (
        <MessageSquare className="size-3 shrink-0" strokeWidth={1.5} />
      )}
      <span className="flex-1 text-left truncate">{thread.title}</span>
      <span className="text-ui-xs shrink-0">{formatRelativeTime(thread.updatedAt)}</span>
    </button>
  );
}
