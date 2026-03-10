import { CollapsibleSection } from "@shared/components";
import { useQuery } from "@shared/hooks/useQuery";
import type { Note } from "@shared/types";
import { FileText } from "lucide-react";
import { useNavigate } from "react-router";

interface NoteSectionProps {
  projectId: string;
  defaultOpen?: boolean;
}

export function NoteSection({ projectId, defaultOpen }: NoteSectionProps) {
  const navigate = useNavigate();
  const { data: notes } = useQuery<Note[]>(
    "note_list_by_entity",
    { entityType: "project", entityId: projectId },
    [],
  );

  return (
    <CollapsibleSection
      title="Notes"
      icon={<FileText className="w-3.5 h-3.5 text-brand" strokeWidth={1.5} />}
      count={notes.length || null}
      defaultOpen={defaultOpen}
    >
      {notes.length === 0 ? (
        <p className="text-[11px] text-dim font-light py-2">No linked notes</p>
      ) : (
        <div className="space-y-0.5">
          {notes.map((note) => (
            <button
              key={note.id}
              type="button"
              onClick={() => navigate(`/notes?noteId=${note.id}`)}
              className="w-full flex items-center gap-2 px-2 py-1.5 rounded-md hover:bg-white/[0.04] transition-colors text-left"
            >
              <FileText className="w-3 h-3 text-brand shrink-0" strokeWidth={1.5} />
              <span className="text-[11px] font-light text-secondary truncate">{note.title}</span>
            </button>
          ))}
        </div>
      )}
    </CollapsibleSection>
  );
}
