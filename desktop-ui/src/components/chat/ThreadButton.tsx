import { Check, MessageSquare, X } from "lucide-react";
import type { ChatThread } from "../../lib/types";

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

function formatRelativeTime(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return "now";
  if (mins < 60) return `${mins}m`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d`;
  if (days < 30) return `${Math.floor(days / 7)}w`;
  return `${Math.floor(days / 30)}mo`;
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
          className="flex-1 min-w-0 bg-white/[0.12] text-primary text-[12px] font-light px-2 py-1 rounded border border-white/[0.08]"
        />
        <button
          type="button"
          onClick={onRenameConfirm}
          aria-label="Confirm rename"
          className="text-success hover:text-success/80 shrink-0"
        >
          <Check className="w-3.5 h-3.5" strokeWidth={2} />
        </button>
        <button
          type="button"
          onClick={onRenameCancel}
          aria-label="Cancel rename"
          className="text-muted hover:text-secondary shrink-0"
        >
          <X className="w-3.5 h-3.5" strokeWidth={2} />
        </button>
      </div>
    );
  }

  return (
    <button
      type="button"
      onClick={() => onSelect(thread.sessionKey)}
      onContextMenu={(e) => onContextMenu(e, thread)}
      className={`w-full flex items-center gap-2 px-3 py-1.5 rounded-lg transition-colors text-[12px] font-light ${
        isActive
          ? "bg-white/[0.12] text-primary"
          : "text-muted hover:bg-white/[0.06] hover:text-secondary"
      }`}
    >
      <MessageSquare className="w-3 h-3 shrink-0" strokeWidth={1.5} />
      <span className="flex-1 text-left truncate">{thread.title}</span>
      <span className="text-[11px] shrink-0">{formatRelativeTime(thread.updatedAt)}</span>
    </button>
  );
}
