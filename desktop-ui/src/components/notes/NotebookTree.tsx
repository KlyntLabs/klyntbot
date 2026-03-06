import { ChevronRight, FolderPlus } from "lucide-react";
import { memo, useMemo, useState } from "react";
import type { Notebook } from "../../lib/types";

interface NotebookTreeProps {
  notebooks: Notebook[];
  selectedId: string | null;
  onSelect: (id: string | null) => void;
  onCreate: () => void;
}

export function NotebookTree({ notebooks, selectedId, onSelect, onCreate }: NotebookTreeProps) {
  const childrenMap = useMemo(() => {
    const map = new Map<string | null, Notebook[]>();
    for (const nb of notebooks) {
      const pid = nb.parentId ?? null;
      const list = map.get(pid) ?? [];
      list.push(nb);
      map.set(pid, list);
    }
    return map;
  }, [notebooks]);

  return (
    <div className="w-48 glass-panel rounded-2xl p-3 flex flex-col gap-1">
      <div className="flex items-center justify-between mb-1">
        <h2 className="text-xs font-medium text-muted uppercase tracking-wider">Notebooks</h2>
        <button
          type="button"
          onClick={onCreate}
          className="w-5 h-5 rounded-md flex items-center justify-center text-dim hover:text-primary hover:bg-white/[0.06] transition-colors"
          aria-label="New notebook"
        >
          <FolderPlus className="w-3.5 h-3.5" />
        </button>
      </div>

      <button
        type="button"
        onClick={() => onSelect(null)}
        className={`text-sm py-1.5 px-2 rounded-lg text-left transition-colors ${
          selectedId === null
            ? "bg-white/[0.08] text-primary"
            : "text-secondary hover:bg-white/[0.04]"
        }`}
      >
        All Notes
      </button>

      {(childrenMap.get(null) ?? []).map((nb) => (
        <TreeNode
          key={nb.id}
          notebook={nb}
          childrenMap={childrenMap}
          selectedId={selectedId}
          onSelect={onSelect}
          depth={0}
        />
      ))}
    </div>
  );
}

interface TreeNodeProps {
  notebook: Notebook;
  childrenMap: Map<string | null, Notebook[]>;
  selectedId: string | null;
  onSelect: (id: string | null) => void;
  depth: number;
}

const TreeNode = memo(function TreeNode({
  notebook,
  childrenMap,
  selectedId,
  onSelect,
  depth,
}: TreeNodeProps) {
  const children = childrenMap.get(notebook.id) ?? [];
  const hasChildren = children.length > 0;
  const [expanded, setExpanded] = useState(true);

  return (
    <div>
      <div className="flex items-center gap-0" style={{ paddingLeft: `${depth * 12}px` }}>
        {hasChildren ? (
          <button
            type="button"
            onClick={() => setExpanded(!expanded)}
            className="w-5 h-7 flex items-center justify-center shrink-0 text-dim hover:text-secondary"
            aria-label={expanded ? "Collapse" : "Expand"}
          >
            <ChevronRight
              className={`w-3 h-3 transition-transform ${expanded ? "rotate-90" : ""}`}
            />
          </button>
        ) : (
          <span className="w-5 shrink-0" />
        )}
        <button
          type="button"
          onClick={() => onSelect(notebook.id)}
          className={`flex-1 min-w-0 text-sm py-1.5 px-1.5 rounded-lg text-left transition-colors flex items-center gap-1 ${
            selectedId === notebook.id
              ? "bg-white/[0.08] text-primary"
              : "text-secondary hover:bg-white/[0.04]"
          }`}
        >
          {notebook.icon && <span className="shrink-0">{notebook.icon}</span>}
          <span className="truncate">{notebook.title}</span>
          <span className="ml-auto text-xs text-dim shrink-0">{notebook.noteCount}</span>
        </button>
      </div>

      {hasChildren && expanded && (
        <div>
          {children.map((child) => (
            <TreeNode
              key={child.id}
              notebook={child}
              childrenMap={childrenMap}
              selectedId={selectedId}
              onSelect={onSelect}
              depth={depth + 1}
            />
          ))}
        </div>
      )}
    </div>
  );
});
