import { Pencil, Trash2 } from "lucide-react";
import { useEffect, useRef } from "react";
import type { ChatThread } from "../../lib/types";

interface ThreadContextMenuProps {
  x: number;
  y: number;
  thread: ChatThread;
  onRename: (thread: ChatThread) => void;
  onDelete: (sessionKey: string) => void;
  onClose: () => void;
}

export function ThreadContextMenu({
  x,
  y,
  thread,
  onRename,
  onDelete,
  onClose,
}: ThreadContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = menuRef.current?.querySelector<HTMLElement>("[role=menuitem]");
    el?.focus();
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    const items = menuRef.current?.querySelectorAll<HTMLElement>("[role=menuitem]");
    if (!items) return;
    const arr = Array.from(items);
    const idx = arr.indexOf(document.activeElement as HTMLElement);
    if (e.key === "ArrowDown") {
      e.preventDefault();
      arr[(idx + 1) % arr.length]?.focus();
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      arr[(idx - 1 + arr.length) % arr.length]?.focus();
    }
    if (e.key === "Escape") onClose();
  };

  return (
    <div
      ref={menuRef}
      role="menu"
      onMouseDown={(e) => e.stopPropagation()}
      onKeyDown={handleKeyDown}
      className="fixed z-50 bg-surface-raised border border-border rounded-lg shadow-lg py-1 min-w-[140px]"
      style={{ left: x, top: y }}
    >
      <button
        type="button"
        role="menuitem"
        onClick={() => onRename(thread)}
        className="w-full flex items-center gap-2 px-3 py-1.5 text-[12px] font-light text-secondary hover:bg-surface-base transition-colors"
      >
        <Pencil className="w-3 h-3" strokeWidth={1.5} />
        Rename
      </button>
      <button
        type="button"
        role="menuitem"
        onClick={() => onDelete(thread.sessionKey)}
        className="w-full flex items-center gap-2 px-3 py-1.5 text-[12px] font-light text-destructive hover:bg-surface-base transition-colors"
      >
        <Trash2 className="w-3 h-3" strokeWidth={1.5} />
        Delete
      </button>
    </div>
  );
}
