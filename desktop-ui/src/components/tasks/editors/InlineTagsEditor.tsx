import { useState, useRef } from 'react';
import { X } from 'lucide-react';
import { Badge } from '../../ui/Badge';

interface InlineTagsEditorProps {
  tags: string[];
  onSave: (tags: string[]) => void;
}

export function InlineTagsEditor({ tags, onSave }: InlineTagsEditorProps) {
  const [editing, setEditing] = useState(false);
  const [input, setInput] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);

  const addTag = () => {
    const trimmed = input.trim().toLowerCase();
    if (trimmed && !tags.includes(trimmed)) {
      onSave([...tags, trimmed]);
    }
    setInput('');
  };

  const removeTag = (tag: string) => {
    onSave(tags.filter(t => t !== tag));
  };

  return (
    <div
      onClick={(e) => { e.stopPropagation(); setEditing(true); }}
      className="flex items-center gap-1 flex-wrap cursor-text rounded px-1 -mx-1 min-h-[24px] transition-colors"
    >
      {tags.map(tag => (
        <span key={tag} className="inline-flex items-center gap-0.5">
          <Badge variant="tag" value={tag} />
          <button
            onClick={(e) => { e.stopPropagation(); removeTag(tag); }}
            className="text-dim hover:text-destructive"
          >
            <X className="w-2.5 h-2.5" />
          </button>
        </span>
      ))}
      {editing ? (
        <input
          ref={inputRef}
          autoFocus
          value={input}
          onChange={e => setInput(e.target.value)}
          onKeyDown={e => {
            if (e.key === 'Enter') { addTag(); }
            if (e.key === 'Escape') { setEditing(false); setInput(''); }
            if (e.key === 'Backspace' && !input && tags.length > 0) {
              removeTag(tags[tags.length - 1]);
            }
          }}
          onBlur={() => { addTag(); setEditing(false); }}
          onClick={e => e.stopPropagation()}
          placeholder="Add tag..."
          className="bg-transparent text-[11px] font-light text-primary outline-none placeholder:text-dim min-w-[60px] flex-1"
        />
      ) : (
        tags.length === 0 && <span className="text-[11px] text-dim">—</span>
      )}
    </div>
  );
}
