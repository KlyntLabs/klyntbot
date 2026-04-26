import { GitBranch, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

interface QuickBridgePopoverProps {
  sourceName: string;
  targetName: string;
  onClose: () => void;
  onCreateNote: (title: string, content: string) => void;
}

export function QuickBridgePopover({
  sourceName,
  targetName,
  onClose,
  onCreateNote,
}: QuickBridgePopoverProps) {
  const [content, setContent] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const title = `Bridge: ${sourceName} \u2194 ${targetName}`;

  // Focus textarea on mount
  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  const handleCreate = () => {
    if (!content.trim()) return;
    onCreateNote(title, content.trim());
    onClose();
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      onClose();
    } else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      handleCreate();
    }
  };

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* biome-ignore lint/a11y/noStaticElementInteractions: backdrop overlay — click to dismiss, keyboard handled by Escape listener */}
      <div className="absolute inset-0 bg-black/30" onClick={onClose} role="presentation" />

      {/* Popover */}
      <div
        className="relative glass-panel rounded-xl w-[400px] shadow-xl p-4"
        role="dialog"
        onKeyDown={handleKeyDown}
      >
        {/* Header */}
        <div className="flex items-center gap-2 mb-3">
          <GitBranch size={14} className="text-brand shrink-0" />
          <h3 className="text-sm font-semibold text-foreground truncate flex-1">{title}</h3>
          <button
            type="button"
            onClick={onClose}
            className="size-6 flex items-center justify-center rounded-md
              text-muted-foreground hover:text-foreground hover:bg-surface-raised/50
              transition-colors"
            aria-label="Close"
          >
            <X size={14} />
          </button>
        </div>

        {/* Connection labels */}
        <div className="flex items-center gap-2 mb-3 text-2xs text-muted-foreground">
          <span className="px-2 py-0.5 rounded-full bg-surface-raised truncate max-w-[160px]">
            {sourceName}
          </span>
          <span className="text-dim">{"\u2194"}</span>
          <span className="px-2 py-0.5 rounded-full bg-surface-raised truncate max-w-[160px]">
            {targetName}
          </span>
        </div>

        {/* Textarea */}
        <textarea
          ref={textareaRef}
          value={content}
          onChange={(e) => setContent(e.target.value)}
          placeholder="How do these connect?"
          rows={3}
          className="w-full bg-surface-base border border-border rounded-lg px-3 py-2
            text-sm text-foreground placeholder:text-dim resize-none
            focus:outline-none focus:ring-1 focus:ring-brand/40"
        />

        {/* Actions */}
        <div className="flex items-center justify-end gap-2 mt-3">
          <button
            type="button"
            onClick={onClose}
            className="px-3 py-1.5 text-xs text-muted-foreground
              hover:text-foreground transition-colors rounded-md"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={handleCreate}
            disabled={!content.trim()}
            className="px-3 py-1.5 text-xs font-medium rounded-md
              bg-brand text-white hover:bg-brand/90 transition-colors
              disabled:opacity-40 disabled:cursor-not-allowed"
          >
            Create
          </button>
        </div>

        <div className="mt-2 text-2xs text-dim text-center">{"\u2318"}+Enter to create</div>
      </div>
    </div>,
    document.body,
  );
}
