import { ipc } from "@shared/hooks/useIpc";
import { invalidateQueries } from "@shared/hooks/useQuery";
import { useCallback, useEffect, useRef, useState } from "react";
import { type AnnotationResponse, useAnnotations } from "../../hooks/useAnnotations";

interface AnnotationSidebarProps {
  noteId: string;
  /** The mark ID of the annotation currently selected in the editor */
  activeAnnotationId: string | null;
  onAnnotationClick: (markId: string) => void;
}

export function AnnotationSidebar({
  noteId,
  activeAnnotationId,
  onAnnotationClick,
}: AnnotationSidebarProps) {
  const { annotations, updateAnnotation, deleteAnnotation } = useAnnotations(noteId, null);

  return (
    <div className="flex h-full flex-col">
      <div className="px-3 py-1.5 text-[10px] text-muted uppercase tracking-wider border-b border-border shrink-0 flex items-center justify-between">
        <span>Annotations ({annotations.length})</span>
      </div>

      <div className="flex-1 overflow-y-auto">
        {annotations.length === 0 ? (
          <div className="flex items-center justify-center h-32 text-xs text-muted">
            Select text in the editor and right-click to annotate.
          </div>
        ) : (
          <div className="flex flex-col">
            {annotations.map((ann) => (
              <AnnotationCard
                key={ann.id}
                annotation={ann}
                isActive={ann.markId === activeAnnotationId || ann.id === activeAnnotationId}
                onClick={() => onAnnotationClick(ann.markId ?? ann.id)}
                onUpdate={updateAnnotation}
                onDelete={deleteAnnotation}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function AnnotationCard({
  annotation,
  isActive,
  onClick,
  onUpdate,
  onDelete,
}: {
  annotation: AnnotationResponse;
  isActive: boolean;
  onClick: () => void;
  onUpdate: (params: { id: string; content?: string; tags?: string }) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
}) {
  const [comment, setComment] = useState(annotation.content || "");
  const [editing, setEditing] = useState(false);
  const cardRef = useRef<HTMLDivElement>(null);

  // Scroll into view when this card becomes active
  useEffect(() => {
    if (isActive && cardRef.current) {
      cardRef.current.scrollIntoView({ behavior: "smooth", block: "nearest" });
    }
  }, [isActive]);

  const handleSaveComment = useCallback(async () => {
    await onUpdate({ id: annotation.id, content: comment });
    setEditing(false);
  }, [annotation.id, comment, onUpdate]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
        handleSaveComment();
      }
      if (e.key === "Escape") {
        setComment(annotation.content || "");
        setEditing(false);
      }
    },
    [handleSaveComment, annotation.content],
  );

  return (
    <div
      ref={cardRef}
      onClick={onClick}
      className={`border-b border-border px-3 py-3 cursor-pointer transition-colors ${
        isActive ? "bg-brand/10 border-l-2 border-l-brand" : "hover:bg-surface-hover"
      }`}
    >
      {/* Quoted text */}
      {annotation.quotedText && (
        <div className="mb-2 border-l-2 border-brand/40 pl-2">
          <p className="text-[11px] text-muted italic leading-relaxed">
            &ldquo;{annotation.quotedText}&rdquo;
          </p>
        </div>
      )}

      {/* Comment / note area */}
      {editing ? (
        <div className="mt-1">
          <textarea
            value={comment}
            onChange={(e) => setComment(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Add a note..."
            className="w-full rounded-md bg-surface-base p-2 text-xs text-primary outline-none ring-1 ring-border focus:ring-brand resize-none"
            rows={3}
            autoFocus
          />
          <div className="mt-1 flex items-center justify-between">
            <span className="text-[9px] text-muted">⌘+Enter to save</span>
            <div className="flex gap-1">
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  setComment(annotation.content || "");
                  setEditing(false);
                }}
                className="rounded px-2 py-0.5 text-[10px] text-muted hover:bg-surface-hover"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  handleSaveComment();
                }}
                className="rounded bg-brand/20 px-2 py-0.5 text-[10px] text-brand hover:bg-brand/30"
              >
                Save
              </button>
            </div>
          </div>
        </div>
      ) : annotation.content ? (
        <p
          className="text-xs text-primary leading-relaxed cursor-text"
          onClick={(e) => {
            e.stopPropagation();
            setEditing(true);
          }}
        >
          {annotation.content}
        </p>
      ) : (
        <button
          type="button"
          className="mt-1 text-[11px] text-muted hover:text-primary transition-colors"
          onClick={(e) => {
            e.stopPropagation();
            setEditing(true);
          }}
        >
          + Add a note...
        </button>
      )}

      {/* Footer: date + delete */}
      <div className="mt-2 flex items-center justify-between">
        <span className="text-[9px] text-muted">
          {new Date(annotation.createdAt).toLocaleString(undefined, {
            month: "short",
            day: "numeric",
            hour: "numeric",
            minute: "2-digit",
          })}
        </span>
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onDelete(annotation.id);
          }}
          className="text-[9px] text-muted hover:text-red-400 transition-colors"
        >
          Delete
        </button>
      </div>
    </div>
  );
}
