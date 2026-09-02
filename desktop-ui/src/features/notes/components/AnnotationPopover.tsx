import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { AnnotationResponse } from "../hooks/useAnnotations";

interface AnnotationPopoverProps {
  annotation: AnnotationResponse;
  position: { top: number; left: number };
  onClose: () => void;
  onEdit: (id: string, content: string) => void;
  onDelete: (id: string) => void;
  onCreateFlashcard: (quotedText: string, content: string) => void;
}

export function AnnotationPopover({
  annotation,
  position,
  onClose,
  onEdit,
  onDelete,
  onCreateFlashcard,
}: AnnotationPopoverProps) {
  const [editing, setEditing] = useState(false);
  const [content, setContent] = useState(annotation.content);
  const popoverRef = useRef<HTMLDivElement>(null);

  // Close on outside click
  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) {
        onClose();
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [onClose]);

  // Close on Escape
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  const handleSave = useCallback(() => {
    onEdit(annotation.id, content);
    setEditing(false);
  }, [annotation.id, content, onEdit]);

  const tags = annotation.tags ? annotation.tags.split(",").filter(Boolean) : [];

  return createPortal(
    <div
      ref={popoverRef}
      className="glass-panel fixed z-50 w-80 rounded-xl p-4 shadow-2xl"
      style={{ top: position.top + 8, left: position.left }}
    >
      {/* Header */}
      <div className="mb-3 flex items-center justify-between">
        <div className="flex items-center gap-2">
          {tags.map((tag) => (
            <span key={tag} className="rounded-full bg-brand/15 px-2 py-0.5 text-ui-xs text-brand">
              {tag}
            </span>
          ))}
        </div>
        <span className="text-ui-xs text-fg-secondary">
          {new Date(annotation.createdAt).toLocaleDateString()}
        </span>
      </div>

      {/* Quoted text */}
      {annotation.quotedText && (
        <div className="mb-3 rounded-md border-l-2 border-brand/50 bg-control-hover/50 px-3 py-2">
          <p className="text-ui-sm text-fg-secondary italic leading-relaxed">"{annotation.quotedText}"</p>
        </div>
      )}

      {/* AI Suggestion */}
      {annotation.aiSuggestion && (
        <div className="mb-3 rounded-md bg-blue-500/10 px-3 py-2">
          <p className="text-ui-xs font-medium text-blue-400 uppercase">AI Insight</p>
          <p className="mt-1 text-ui-sm text-brand">{annotation.aiSuggestion}</p>
        </div>
      )}

      {/* Annotation content */}
      {editing ? (
        <div className="mb-3">
          <textarea
            value={content}
            onChange={(e) => setContent(e.target.value)}
            className="w-full rounded-md bg-bg-elevated p-2 text-ui-sm text-brand outline-none ring-1 ring-separator focus:ring-fg-secondary/30"
            rows={3}
          />
          <div className="mt-1 flex justify-end gap-1">
            <button
              type="button"
              onClick={() => setEditing(false)}
              className="rounded px-2 py-1 text-ui-xs text-fg-secondary hover:bg-control-hover"
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={handleSave}
              className="rounded bg-brand/20 px-2 py-1 text-ui-xs text-brand hover:bg-brand/30"
            >
              Save
            </button>
          </div>
        </div>
      ) : (
        annotation.content && (
          <p className="mb-3 text-ui-sm text-brand leading-relaxed">{annotation.content}</p>
        )
      )}

      {/* Action buttons */}
      <div className="flex items-center gap-1 border-t border-separator pt-2">
        <ActionButton
          onClick={() => onCreateFlashcard(annotation.quotedText ?? "", annotation.content)}
        >
          Flashcard
        </ActionButton>
        <ActionButton onClick={() => setEditing(true)}>Edit</ActionButton>
        <ActionButton
          onClick={() => onDelete(annotation.id)}
          className="text-red-400 hover:bg-red-500/10"
        >
          Delete
        </ActionButton>
      </div>
    </div>,
    document.body,
  );
}

function ActionButton({
  onClick,
  children,
  className = "",
}: {
  onClick: () => void;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-md px-2 py-1 text-ui-xs text-fg-secondary hover:bg-control-hover ${className}`}
    >
      {children}
    </button>
  );
}
