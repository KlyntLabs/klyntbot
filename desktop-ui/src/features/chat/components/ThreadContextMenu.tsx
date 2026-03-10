import type { ChatThread } from "@shared/types";
import { ContextMenu, ContextMenuItem, ContextMenuSeparator } from "@shared/ui";
import { Pencil, Trash2 } from "lucide-react";

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
  return (
    <ContextMenu x={x} y={y} onClose={onClose}>
      <ContextMenuItem icon={<Pencil className="w-4 h-4" />} onClick={() => onRename(thread)}>
        Rename
      </ContextMenuItem>
      <ContextMenuSeparator />
      <ContextMenuItem
        icon={<Trash2 className="w-4 h-4" />}
        onClick={() => onDelete(thread.sessionKey)}
        destructive
      >
        Delete
      </ContextMenuItem>
    </ContextMenu>
  );
}
