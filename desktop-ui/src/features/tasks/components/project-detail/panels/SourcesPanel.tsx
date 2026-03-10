import { useProjectSources } from "@shared/hooks";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import type { Note, ProjectSource, ProjectSourceCreateParams } from "@shared/types";
import { ExternalLink, FileText, Link2, Plus, Trash2, X } from "lucide-react";
import { useCallback, useState } from "react";

interface SourcesPanelProps {
  projectId: string;
  onClose: () => void;
}

export function SourcesPanel({ projectId, onClose }: SourcesPanelProps) {
  const { data: sources, refetch: refetchSources } = useProjectSources(projectId);
  const { data: linkedNotes } = useQuery<Note[]>(
    "note_list_by_entity",
    { entityType: "project", entityId: projectId },
    [],
  );

  const createSource = useMutation<ProjectSource, ProjectSourceCreateParams>(
    "project_source_create",
    "params",
  );
  const deleteSource = useMutation<boolean, { id: string }>("project_source_delete");

  const [adding, setAdding] = useState(false);
  const [newTitle, setNewTitle] = useState("");
  const [newUrl, setNewUrl] = useState("");

  const handleAdd = useCallback(async () => {
    if (!newTitle.trim()) return;
    await createSource.mutate({
      projectId,
      sourceType: newUrl.trim() ? "link" : "snippet",
      title: newTitle.trim(),
      url: newUrl.trim() || undefined,
    });
    setNewTitle("");
    setNewUrl("");
    setAdding(false);
    refetchSources();
  }, [projectId, newTitle, newUrl, createSource, refetchSources]);

  const handleDelete = useCallback(
    async (id: string) => {
      await deleteSource.mutate({ id });
      refetchSources();
    },
    [deleteSource, refetchSources],
  );

  return (
    <div className="w-80 border-r border-white/[0.06] flex flex-col overflow-hidden shrink-0">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-white/[0.06]">
        <h3 className="text-[13px] font-medium text-primary">Sources</h3>
        <button
          type="button"
          onClick={onClose}
          className="text-muted hover:text-secondary transition-colors"
        >
          <X className="w-4 h-4" strokeWidth={1.5} />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto">
        {/* Linked notes (auto-included) */}
        {linkedNotes.length > 0 && (
          <div className="p-4 border-b border-white/[0.06]">
            <p className="text-[10px] font-medium text-dim uppercase tracking-wider mb-2">
              From Project Notes (auto-included)
            </p>
            <div className="space-y-1">
              {linkedNotes.map((note) => (
                <div key={note.id} className="flex items-center gap-2 px-2 py-1.5">
                  <FileText className="w-3 h-3 text-brand shrink-0" strokeWidth={1.5} />
                  <span className="text-[12px] font-light text-secondary truncate">
                    {note.title}
                  </span>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* External sources */}
        <div className="p-4">
          <div className="flex items-center justify-between mb-2">
            <p className="text-[10px] font-medium text-dim uppercase tracking-wider">
              External Sources
            </p>
            <button
              type="button"
              onClick={() => setAdding(true)}
              className="text-muted hover:text-brand transition-colors"
            >
              <Plus className="w-3.5 h-3.5" strokeWidth={1.5} />
            </button>
          </div>

          {adding && (
            <div className="mb-3 space-y-2">
              <input
                value={newTitle}
                onChange={(e) => setNewTitle(e.target.value)}
                placeholder="Title"
                className="w-full bg-white/[0.04] rounded-md px-3 py-2 text-[12px] font-light text-primary placeholder:text-dim outline-none border border-white/[0.06] focus:border-brand/40 transition-colors"
                onKeyDown={(e) => {
                  if (e.key === "Enter") handleAdd();
                  if (e.key === "Escape") setAdding(false);
                }}
              />
              <input
                value={newUrl}
                onChange={(e) => setNewUrl(e.target.value)}
                placeholder="URL (optional)"
                className="w-full bg-white/[0.04] rounded-md px-3 py-2 text-[12px] font-light text-primary placeholder:text-dim outline-none border border-white/[0.06] focus:border-brand/40 transition-colors"
                onKeyDown={(e) => {
                  if (e.key === "Enter") handleAdd();
                  if (e.key === "Escape") setAdding(false);
                }}
              />
              <div className="flex gap-2">
                <button
                  type="button"
                  onClick={handleAdd}
                  className="px-3 py-1.5 rounded-md bg-brand text-white text-[11px] font-medium"
                >
                  Add
                </button>
                <button
                  type="button"
                  onClick={() => setAdding(false)}
                  className="px-3 py-1.5 rounded-md text-muted text-[11px] font-light hover:text-secondary"
                >
                  Cancel
                </button>
              </div>
            </div>
          )}

          <div className="space-y-1">
            {sources.map((source) => (
              <div
                key={source.id}
                className="flex items-center gap-2 px-2 py-1.5 rounded-md hover:bg-white/[0.04] group"
              >
                {source.url ? (
                  <ExternalLink className="w-3 h-3 text-muted shrink-0" strokeWidth={1.5} />
                ) : (
                  <Link2 className="w-3 h-3 text-muted shrink-0" strokeWidth={1.5} />
                )}
                <span className="text-[12px] font-light text-secondary truncate flex-1">
                  {source.title}
                </span>
                <button
                  type="button"
                  onClick={() => handleDelete(source.id)}
                  className="opacity-0 group-hover:opacity-100 text-muted hover:text-destructive transition-all"
                >
                  <Trash2 className="w-3 h-3" strokeWidth={1.5} />
                </button>
              </div>
            ))}
            {sources.length === 0 && !adding && (
              <p className="text-[11px] text-dim font-light py-3 text-center">
                No external sources
              </p>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
