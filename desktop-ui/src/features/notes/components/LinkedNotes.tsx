import { useQuery } from "@shared/hooks/useQuery";
import type { NoteListItem } from "@shared/types";
import { FileText } from "lucide-react";
import { useNavigate } from "react-router";

interface LinkedNotesProps {
  entityType: string;
  entityId: string;
}

export function LinkedNotes({ entityType, entityId }: LinkedNotesProps) {
  const navigate = useNavigate();
  const { data: notes } = useQuery<NoteListItem[]>(
    "note_list_by_entity",
    { entityType, entityId },
    [],
  );

  if (notes.length === 0) return null;

  return (
    <div>
      <h3 className="text-ui-sm font-light text-fg-secondary uppercase tracking-wider mb-3">
        Linked Notes
      </h3>
      <div className="island overflow-hidden">
        {notes.map((note) => (
          <button
            key={note.id}
            type="button"
            onClick={() => navigate(`/notes?noteId=${note.id}`)}
            className="w-full flex items-center gap-2.5 px-4 py-2.5 text-left hover:bg-control-hover transition-colors border-b border-separator last:border-b-0"
          >
            <FileText className="size-3.5 text-brand shrink-0" strokeWidth={1.5} />
            <span className="text-ui font-light text-fg-secondary truncate">{note.title}</span>
            {note.tags.length > 0 && (
              <span className="text-ui-xs text-fg-dim ml-auto shrink-0">
                {note.tags.slice(0, 2).join(", ")}
              </span>
            )}
          </button>
        ))}
      </div>
    </div>
  );
}
