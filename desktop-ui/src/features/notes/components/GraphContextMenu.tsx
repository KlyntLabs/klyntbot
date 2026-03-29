import { useDismiss } from "@shared/hooks";
import {
  BookOpen,
  Copy,
  EyeOff,
  FileSearch,
  GitBranch,
  GraduationCap,
  Link2,
  Network,
} from "lucide-react";
import { useRef } from "react";
import { createPortal } from "react-dom";
import type { ForceNode, ForceNodeType } from "../hooks/useGraphElements";

export type ContextMenuAction =
  | "open_in_editor"
  | "expand_tree"
  | "find_related"
  | "quick_bridge"
  | "focus_community"
  | "pin_to_focus"
  | "view_members"
  | "find_across_notes"
  | "open_references"
  | "hide_from_graph"
  | "open_at_heading"
  | "create_flashcard";

interface MenuItem {
  action: ContextMenuAction;
  label: string;
  icon: React.ReactNode;
}

const MENU_BY_TYPE: Record<ForceNodeType, MenuItem[]> = {
  note: [
    { action: "open_in_editor", label: "Open in editor", icon: <BookOpen size={12} /> },
    { action: "expand_tree", label: "Expand tree", icon: <Network size={12} /> },
    { action: "find_related", label: "Find related", icon: <FileSearch size={12} /> },
    { action: "quick_bridge", label: "Quick bridge", icon: <GitBranch size={12} /> },
  ],
  entity: [
    { action: "find_across_notes", label: "Find across notes", icon: <Copy size={12} /> },
    { action: "open_references", label: "Open references", icon: <Link2 size={12} /> },
    { action: "hide_from_graph", label: "Hide from graph", icon: <EyeOff size={12} /> },
  ],
  tree_section: [
    { action: "open_at_heading", label: "Open at heading", icon: <BookOpen size={12} /> },
    { action: "create_flashcard", label: "Create flashcard", icon: <GraduationCap size={12} /> },
  ],
  tree_text: [],
};

interface GraphContextMenuProps {
  node: ForceNode;
  position: { x: number; y: number };
  onAction: (action: ContextMenuAction, node: ForceNode) => void;
  onClose: () => void;
}

export function GraphContextMenu({ node, position, onAction, onClose }: GraphContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  useDismiss(menuRef, onClose, true);

  const items = MENU_BY_TYPE[node.nodeType] ?? [];

  if (items.length === 0) return null;

  // Clamp position to stay within viewport
  const menuWidth = 180;
  const menuHeight = items.length * 32 + 16; // rough estimate
  const x = Math.min(position.x, window.innerWidth - menuWidth - 8);
  const y = Math.min(position.y, window.innerHeight - menuHeight - 8);

  return createPortal(
    <div
      ref={menuRef}
      className="fixed z-50 glass-panel rounded-lg py-1 min-w-[180px] shadow-xl"
      style={{ left: x, top: y }}
    >
      <div className="px-2.5 py-1.5 text-2xs text-dim truncate border-b border-border-subtle mb-1">
        {node.label}
      </div>
      {items.map((item) => (
        <button
          key={item.action}
          type="button"
          className="flex items-center gap-2 w-full px-2.5 py-1.5 text-xs text-foreground
            hover:bg-surface-raised/50 transition-colors text-left"
          onClick={() => {
            onAction(item.action, node);
            onClose();
          }}
        >
          <span className="text-muted-foreground">{item.icon}</span>
          {item.label}
        </button>
      ))}
    </div>,
    document.body,
  );
}
