import { ipc } from "@shared/hooks/useIpc";
import type { TreePathRef } from "@shared/types/tree-path";

interface TreePathBreadcrumbProps {
  treePaths: TreePathRef[];
}

function navigateToSection(noteId: string, nodeId: string) {
  ipc("navigate_to_note_section", { noteId, nodeId }).catch((err) => {
    console.warn("navigate_to_note_section not implemented yet:", err);
  });
}

export function TreePathBreadcrumb({ treePaths }: TreePathBreadcrumbProps) {
  if (treePaths.length === 0) return null;

  return (
    <div className="flex flex-wrap gap-1.5 mt-2">
      {treePaths.map((ref) => (
        <button
          key={`${ref.noteId}-${ref.nodeId}`}
          type="button"
          onClick={() => navigateToSection(ref.noteId, ref.nodeId)}
          className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md bg-glass-subtle/50
            text-[10px] text-fg-secondary hover:text-fg transition-colors cursor-pointer
            border border-separator/30 hover:border-separator/60"
          title={`${ref.noteName} — ${Math.round(ref.similarity * 100)}% match`}
        >
          <span className="shrink-0">📄</span>
          {ref.path.map((segment, i) => (
            <span key={segment.nodeId} className="inline-flex items-center gap-1">
              {i > 0 && <span className="text-fg-dim">›</span>}
              <span>{segment.title}</span>
            </span>
          ))}
        </button>
      ))}
    </div>
  );
}
