import * as ContextMenu from "@radix-ui/react-context-menu";
import type { ReactNode } from "react";

interface EditorContextMenuProps {
  children: ReactNode;
  hasSelection: boolean;
  onAnnotate: () => void;
  onFlashcard: () => void;
  onTranslate: () => void;
  onAskAI: () => void;
  onLinkedView: () => void;
  onApplyPerspective: (type: string) => void;
}

export function EditorContextMenu({
  children,
  hasSelection,
  onAnnotate,
  onFlashcard,
  onTranslate,
  onAskAI,
  onLinkedView,
  onApplyPerspective,
}: EditorContextMenuProps) {
  return (
    <ContextMenu.Root>
      <ContextMenu.Trigger asChild>{children}</ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Content className="glass-panel min-w-[200px] rounded-lg p-1.5 shadow-xl">
          {hasSelection && (
            <>
              <ContextMenu.Label className="px-2 py-1 text-[11px] font-medium text-muted uppercase tracking-wide">
                Selection
              </ContextMenu.Label>
              <MenuItem onClick={onAnnotate} shortcut="⌥A">
                Annotate
              </MenuItem>
              <MenuItem onClick={onFlashcard} shortcut="⌥F">
                Create Flashcard
              </MenuItem>
              <MenuItem onClick={onTranslate}>Translate</MenuItem>
              <ContextMenu.Separator className="my-1 h-px bg-border" />
            </>
          )}

          <ContextMenu.Label className="px-2 py-1 text-[11px] font-medium text-muted uppercase tracking-wide">
            AI Actions
          </ContextMenu.Label>
          <MenuItem onClick={onAskAI}>Ask AI</MenuItem>
          <MenuItem onClick={onLinkedView} shortcut="⌥L">
            Linked View
          </MenuItem>

          <ContextMenu.Separator className="my-1 h-px bg-border" />

          <ContextMenu.Sub>
            <ContextMenu.SubTrigger className="flex items-center justify-between rounded-md px-2 py-1.5 text-xs text-primary outline-none select-none data-[highlighted]:bg-surface-hover">
              Apply Perspective
              <span className="text-muted ml-4">▸</span>
            </ContextMenu.SubTrigger>
            <ContextMenu.Portal>
              <ContextMenu.SubContent className="glass-panel min-w-[160px] rounded-lg p-1.5 shadow-xl">
                <MenuItem onClick={() => onApplyPerspective("linked-view")}>Linked View</MenuItem>
                <MenuItem onClick={() => onApplyPerspective("annotated")}>Annotated</MenuItem>
                <MenuItem onClick={() => onApplyPerspective("study-mode")}>Study Mode</MenuItem>
              </ContextMenu.SubContent>
            </ContextMenu.Portal>
          </ContextMenu.Sub>
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  );
}

function MenuItem({
  onClick,
  shortcut,
  children,
}: {
  onClick: () => void;
  shortcut?: string;
  children: ReactNode;
}) {
  return (
    <ContextMenu.Item
      onClick={onClick}
      className="flex items-center justify-between rounded-md px-2 py-1.5 text-xs text-primary outline-none select-none data-[highlighted]:bg-surface-hover"
    >
      <span>{children}</span>
      {shortcut && <span className="ml-4 text-muted text-[10px]">{shortcut}</span>}
    </ContextMenu.Item>
  );
}
