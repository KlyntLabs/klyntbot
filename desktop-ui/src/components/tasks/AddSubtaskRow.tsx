import { useState, useCallback } from 'react';
import { Plus } from 'lucide-react';

interface AddSubtaskRowProps {
  parentId: string;
  showArea: boolean;
  onCreate: (parentId: string, title: string) => void;
}

export function AddSubtaskRow({ parentId, showArea, onCreate }: AddSubtaskRowProps) {
  const [editing, setEditing] = useState(false);
  const [title, setTitle] = useState('');

  const colCount = showArea ? 8 : 7;

  const save = useCallback(() => {
    if (title.trim()) {
      onCreate(parentId, title.trim());
      setTitle('');
    }
    setEditing(false);
  }, [title, parentId, onCreate]);

  return (
    <tr
      className="border-b border-border-subtle last:border-b-0 bg-surface-lowest"
      style={{ boxShadow: 'inset 3px 0 0 var(--brand)' }}
    >
      <td className="px-5 py-1.5 w-9" />
      <td colSpan={colCount - 1} className="px-5 py-1.5" onClick={e => e.stopPropagation()}>
        <div className="flex items-center" style={{ paddingLeft: 24 }}>
          {editing ? (
            <input
              autoFocus
              value={title}
              onChange={e => setTitle(e.target.value)}
              onKeyDown={e => {
                if (e.key === 'Enter') save();
                if (e.key === 'Escape') { setEditing(false); setTitle(''); }
              }}
              onBlur={save}
              placeholder="Subtask title..."
              className="bg-transparent text-[12px] font-light text-primary outline-none placeholder:text-dim flex-1"
            />
          ) : (
            <button
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
