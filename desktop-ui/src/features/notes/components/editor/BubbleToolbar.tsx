import type { Editor } from "@tiptap/react";
import { BubbleMenu } from "@tiptap/react/menus";

interface BubbleToolbarProps {
  editor: Editor;
  onAnnotate: () => void;
  onFlashcard: () => void;
  onTranslate: () => void;
  onAskAI: () => void;
}

export function BubbleToolbar({
  editor,
  onAnnotate,
  onFlashcard,
  onTranslate,
  onAskAI,
}: BubbleToolbarProps) {
  return (
    <BubbleMenu
      editor={editor}
      tippyOptions={{ duration: 150, delay: [200, 0] }}
      shouldShow={({ editor, state }) => {
        if (state.selection.empty) return false;
        if (editor.isActive("codeBlock")) return false;
        return true;
      }}
    >
      <div className="glass-panel flex items-center gap-0.5 rounded-[10px] p-1.5 shadow-lg">
        <ToolbarButton onClick={onAnnotate} label="Annotate" shortcut="⌥A" className="text-brand" />
        <ToolbarButton
          onClick={onFlashcard}
          label="Flashcard"
          shortcut="⌥F"
          className="text-purple-400"
        />
        <ToolbarButton onClick={onTranslate} label="Translate" className="text-green-400" />
        <ToolbarButton onClick={onAskAI} label="Ask AI" className="text-blue-400" />
      </div>
    </BubbleMenu>
  );
}

function ToolbarButton({
  onClick,
  label,
  shortcut,
  className = "",
}: {
  onClick: () => void;
  label: string;
  shortcut?: string;
  className?: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`flex items-center gap-1 rounded-md px-2.5 py-1.5 text-ui-sm hover:bg-control-hover ${className}`}
      title={shortcut ? `${label} (${shortcut})` : label}
    >
      {label}
    </button>
  );
}
