import type { InboxItem } from "@shared/types";
import { ChevronRight, FilePlus, Inbox, Trash2 } from "lucide-react";
import { useCallback, useState } from "react";

interface InboxSectionProps {
  items: InboxItem[];
  onCreateAsNote: (content: string) => void;
  onDiscard: (id: string) => void;
}

export function InboxSection({ items, onCreateAsNote, onDiscard }: InboxSectionProps) {
  const [isOpen, setIsOpen] = useState(false);

  const handleCreateAsNote = useCallback(
    (item: InboxItem) => {
      onCreateAsNote(item.content);
      onDiscard(item.id);
    },
    [onCreateAsNote, onDiscard],
  );

  if (items.length === 0) return null;

  return (
    <div className="flex flex-col gap-0.5">
      <button
        type="button"
        onClick={() => setIsOpen((prev) => !prev)}
        className="flex items-center gap-1 px-2 py-1 text-[10px] uppercase tracking-wider text-dim hover:text-muted transition-colors"
      >
        <ChevronRight className={`w-3 h-3 transition-transform ${isOpen ? "rotate-90" : ""}`} />
        <Inbox className="w-3 h-3" />
        <span>Inbox</span>
        <span className="ml-auto text-[9px] bg-brand/20 text-brand px-1.5 py-px rounded-full font-medium">
          {items.length}
        </span>
      </button>

      {isOpen && (
        <div className="flex flex-col gap-0.5 px-1">
          {items.map((item) => (
            <InboxItemRow
              key={item.id}
              item={item}
              onCreateAsNote={() => handleCreateAsNote(item)}
              onDiscard={() => onDiscard(item.id)}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function InboxItemRow({
  item,
  onCreateAsNote,
  onDiscard,
}: {
  item: InboxItem;
  onCreateAsNote: () => void;
  onDiscard: () => void;
}) {
  return (
    <div className="flex flex-col gap-1 px-2 py-1.5 rounded-lg hover:bg-accent transition-colors group">
      <span className="text-xs text-muted-foreground line-clamp-2">{item.content}</span>
      <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
        <button
          type="button"
          onClick={onCreateAsNote}
          className="flex items-center gap-1 text-[10px] text-brand hover:text-brand/80 transition-colors"
          title="Create as note"
        >
          <FilePlus size={11} />
          <span>Note</span>
        </button>
        <button
          type="button"
          onClick={onDiscard}
          className="flex items-center gap-1 text-[10px] text-dim hover:text-destructive transition-colors ml-2"
          title="Discard"
        >
          <Trash2 size={11} />
          <span>Discard</span>
        </button>
      </div>
    </div>
  );
}
