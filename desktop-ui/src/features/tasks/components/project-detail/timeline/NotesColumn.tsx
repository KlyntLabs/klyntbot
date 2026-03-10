import { useQuery } from "@shared/hooks/useQuery";
import type { Note } from "@shared/types";
import { FileText } from "lucide-react";
import { useNavigate } from "react-router";

interface NotesColumnProps {
  projectId: string;
}

export function NotesColumn({ projectId }: NotesColumnProps) {
  const navigate = useNavigate();
  const { data: notes } = useQuery<Note[]>(
    "note_list_by_entity",
    { entityType: "project", entityId: projectId },
    [],
  );

  if (notes.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-[11px] text-dim font-light">
        No notes
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-0.5 p-2">
      {notes.map((note) => (
        <button
          key={note.id}
          type="button"
          onClick={() => navigate(`/notes?noteId=${note.id}`)}
          className="flex items-center gap-2 px-2.5 py-1.5 rounded-md hover:bg-white/[0.04] transition-colors text-left"
        >
          <FileText className="w-3 h-3 text-brand shrink-0" strokeWidth={1.5} />
          <span className="text-[11px] font-light text-secondary truncate">{note.title}</span>
        </button>
      ))}
    </div>
  );
}
