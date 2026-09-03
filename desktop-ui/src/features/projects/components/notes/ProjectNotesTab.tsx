import type { NoteListItem } from "@shared/types";
import { ExternalLink } from "lucide-react";
import { useCallback, useMemo, useState } from "react";
import { useNavigate } from "react-router";
import { useProjectContext } from "../../contexts/ProjectContext";
import { useProjectNotes } from "../../hooks/useProjectNotes";
import { NoteActionBar } from "./NoteActionBar";
import { NoteSidebar } from "./NoteSidebar";

export function ProjectNotesTab() {
  const { project } = useProjectContext();
  const navigate = useNavigate();
  const [selectedNoteId, setSelectedNoteId] = useState<string | null>(null);

  const notebookIds = useMemo(() => {
    const settings = project?.settings as Record<string, unknown> | undefined;
    const ids = settings?.notebookIds;
    return Array.isArray(ids) ? (ids as string[]) : [];
  }, [project?.settings]);

  const { data: notes, loading, refetch } = useProjectNotes(notebookIds);

  // note_list returns NoteListItem; body fields may be absent
  const selectedNote = useMemo(
    () =>
      (notes.find((n) => n.id === selectedNoteId) ?? null) as
        | (NoteListItem & { body?: string; bodyHtml?: string | null })
        | null,
    [notes, selectedNoteId],
  );

  const handleSelectNote = useCallback((noteId: string) => {
    setSelectedNoteId(noteId);
  }, []);

  const handleOpenInNotes = useCallback(() => {
    if (selectedNoteId) {
      navigate(`/notes?noteId=${selectedNoteId}`);
    }
  }, [selectedNoteId, navigate]);

  if (!project) return null;

  return (
    <div className="flex h-full">
      {/* Left sidebar */}
      <NoteSidebar
        notebookIds={notebookIds}
        notes={notes}
        loading={loading}
        selectedNoteId={selectedNoteId}
        onSelectNote={handleSelectNote}
      />

      {/* Right content */}
      <div className="flex-1 flex flex-col min-w-0">
        {selectedNote ? (
          <>
            {/* Note header */}
            <div className="flex items-center gap-3 px-4 py-3 border-b border-separator">
              <h2 className="text-sm font-semibold text-fg truncate">
                {selectedNote.title || "Untitled"}
              </h2>
              <span className="text-ui-xs text-fg-secondary">
                Updated {new Date(selectedNote.updatedAt).toLocaleDateString()}
              </span>
              {selectedNote.tags.length > 0 && (
                <div className="flex gap-1 ml-2">
                  {selectedNote.tags.slice(0, 3).map((tag) => (
                    <span
                      key={tag}
                      className="text-ui-xs px-1.5 py-0.5 rounded bg-control-hover text-fg-secondary"
                    >
                      {tag}
                    </span>
                  ))}
                </div>
              )}
              <button
                type="button"
                onClick={handleOpenInNotes}
                className="ml-auto flex items-center gap-1 text-ui-sm text-brand hover:text-brand/80 transition-colors"
              >
                <ExternalLink className="size-3.5" />
                Open in Notes
              </button>
            </div>

            {/* Note body (read-only preview) */}
            <div className="flex-1 overflow-y-auto px-6 py-4">
              {selectedNote.bodyHtml ? (
                <div
                  className="prose prose-sm prose-invert max-w-none text-fg"
                  // biome-ignore lint/security/noDangerouslySetInnerHtml: read-only note preview
                  dangerouslySetInnerHTML={{
                    __html: selectedNote.bodyHtml,
                  }}
                />
              ) : (
                <div className="text-sm text-fg-secondary whitespace-pre-wrap">
                  {selectedNote.body || "Empty note"}
                </div>
              )}
            </div>

            {/* Action bar */}
            <NoteActionBar
              noteId={selectedNote.id}
              noteTitle={selectedNote.title}
              onInsightGenerated={refetch}
            />
          </>
        ) : (
          /* Empty state */
          <div className="flex-1 flex flex-col items-center justify-center text-center px-6">
            {notes.length === 0 && !loading ? (
              <>
                <p className="text-sm text-fg-secondary mb-2">No notes linked to this project.</p>
                <p className="text-ui-sm text-fg-secondary">
                  Create a note or link an existing notebook to get started.
                </p>
              </>
            ) : (
              <p className="text-sm text-fg-secondary">
                Select a note from the sidebar to view it.
              </p>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
