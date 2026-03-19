import { useAnnotations } from "../../hooks/useAnnotations";

interface AnnotatedViewProps {
  noteId: string;
  sectionId: string;
}

export function AnnotatedView({ noteId, sectionId: _ }: AnnotatedViewProps) {
  const { annotations } = useAnnotations(noteId, null);

  if (annotations.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-xs text-muted">
        No annotations yet. Select text and press ⌥A to annotate.
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col gap-3 overflow-y-auto p-4">
      <h3 className="text-xs font-medium text-muted">
        {annotations.length} annotation{annotations.length !== 1 ? "s" : ""}
      </h3>

      {annotations.map((ann) => (
        <div
          key={ann.id}
          className="rounded-lg border border-brand/20 bg-brand/5 p-3 transition-colors hover:bg-brand/10"
        >
          {ann.quotedText && (
            <div className="mb-2 border-l-2 border-brand/50 pl-2">
              <p className="text-[11px] text-muted italic">"{ann.quotedText}"</p>
            </div>
          )}

          {ann.content && <p className="text-xs text-primary">{ann.content}</p>}

          <div className="mt-2 flex items-center justify-between">
            <div className="flex gap-1">
              {ann.tags &&
                ann.tags
                  .split(",")
                  .filter(Boolean)
                  .map((tag) => (
                    <span
                      key={tag}
                      className="rounded-full bg-surface-hover px-1.5 py-0.5 text-[9px] text-muted"
                    >
                      {tag}
                    </span>
                  ))}
            </div>
            <span className="text-[10px] text-muted">
              {new Date(ann.createdAt).toLocaleDateString()}
            </span>
          </div>
        </div>
      ))}
    </div>
  );
}
