import { Check, ClipboardCopy, Pencil, Send } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

interface ActionBarProps {
  content: string;
  onEdit: (newContent: string) => void;
  onSend: (content: string) => void;
}

export function ActionBar({ content, onEdit, onSend }: ActionBarProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [copied, setCopied] = useState(false);
  const copyTimerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => {
    return () => clearTimeout(copyTimerRef.current);
  }, []);

  const handleCopy = useCallback(async () => {
    await navigator.clipboard.writeText(content);
    setCopied(true);
    clearTimeout(copyTimerRef.current);
    copyTimerRef.current = setTimeout(() => setCopied(false), 2000);
  }, [content]);

  const handleEditToggle = useCallback(() => {
    if (editing) {
      onEdit(draft);
      setEditing(false);
    } else {
      setDraft(content);
      setEditing(true);
    }
  }, [editing, draft, content, onEdit]);

  return (
    <div className="space-y-2">
      {editing && (
        <textarea
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          className="w-full glass-input px-3 py-2 text-[12px] text-primary font-light resize-none outline-none"
          rows={4}
        />
      )}
      <div className="flex items-center gap-1">
        <button
          type="button"
          onClick={handleCopy}
          className="flex items-center gap-1 px-2 py-1 rounded-md text-[11px] font-light text-muted hover:text-secondary hover:bg-white/[0.06] transition-colors"
        >
          {copied ? (
            <Check className="w-3 h-3" strokeWidth={1.5} />
          ) : (
            <ClipboardCopy className="w-3 h-3" strokeWidth={1.5} />
          )}
          {copied ? "Copied" : "Copy"}
        </button>
        <button
          type="button"
          onClick={handleEditToggle}
          className="flex items-center gap-1 px-2 py-1 rounded-md text-[11px] font-light text-muted hover:text-secondary hover:bg-white/[0.06] transition-colors"
        >
          <Pencil className="w-3 h-3" strokeWidth={1.5} />
          {editing ? "Done" : "Edit"}
        </button>
        <button
          type="button"
          onClick={() => onSend(editing ? draft : content)}
          className="flex items-center gap-1 px-2 py-1 rounded-md text-[11px] font-light text-muted hover:text-secondary hover:bg-white/[0.06] transition-colors"
        >
          <Send className="w-3 h-3" strokeWidth={1.5} />
          Send
        </button>
      </div>
    </div>
  );
}
