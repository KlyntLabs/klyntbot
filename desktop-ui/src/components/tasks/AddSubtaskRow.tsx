import { Plus } from "lucide-react";
import { useCallback, useState } from "react";
import { useTaskTable } from "./TaskTableContext";

interface AddSubtaskRowProps {
  parentId: string;
}

export function AddSubtaskRow({ parentId }: AddSubtaskRowProps) {
  const { showArea, onCreateSubtask } = useTaskTable();
  const [editing, setEditing] = useState(false);
  const [title, setTitle] = useState("");

  const colCount = showArea ? 8 : 7;

  const save = useCallback(() => {
    if (title.trim()) {
      onCreateSubtask(parentId, title.trim());
      setTitle("");
    }
    setEditing(false);
  }, [title, parentId, onCreateSubtask]);

  return (
    <tr
      className="border-b border-border-subtle last:border-b-0 bg-surface-lowest"
      style={{ boxShadow: "inset 3px 0 0 var(--brand)" }}
    >
      <td className="px-5 py-1.5 w-9" />
      <td
        colSpan={colCount - 1}
        className="px-5 py-1.5"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center pl-6">
          {editing ? (
            <input
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") save();
                if (e.key === "Escape") {
                  setEditing(false);
                  setTitle("");
                }
              }}
              onBlur={save}
              placeholder="Subtask title..."
              className="bg-transparent text-[12px] font-light text-primary outline-none placeholder:text-dim flex-1"
            />
          ) : (
            <button
              type="button"
              onClick={() => setEditing(true)}
              className="flex items-center gap-1 text-[12px] font-light text-dim hover:text-muted transition-colors"
            >
              <Plus className="w-3 h-3" />
              Add subtask
            </button>
          )}
        </div>
      </td>
    </tr>
  );
}
