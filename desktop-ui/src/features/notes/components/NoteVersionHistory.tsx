import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import { formatRelativeTime } from "@shared/lib/dates";
import type { Note, NoteVersion } from "@shared/types";
import { RotateCcw } from "lucide-react";
import { useCallback, useState } from "react";

interface NoteVersionHistoryProps {
  noteId: string;
  onRestore: (restored: Note) => void;
}

export function NoteVersionHistory({ noteId, onRestore }: NoteVersionHistoryProps) {
  const { data: versions, refetch } = useQuery<NoteVersion[]>("note_version_list", { noteId }, []);
  const [previewId, setPreviewId] = useState<string | null>(null);
  const [restoring, setRestoring] = useState(false);
  const [restoreError, setRestoreError] = useState<string | null>(null);

  const previewVersion = versions.find((v) => v.id === previewId);

  const handleRestore = useCallback(
    async (versionId: string) => {
      setRestoring(true);
      setRestoreError(null);
      try {
        const restored = await ipc<Note>("note_version_restore", { versionId, noteId });
        onRestore(restored);
        refetch();
        setPreviewId(null);
      } catch (e) {
        console.error("Failed to restore version:", e);
        setRestoreError(e instanceof Error ? e.message : "Failed to restore version");
      } finally {
        setRestoring(false);
      }
    },
    [noteId, onRestore, refetch],
  );

  return (
    <div className="w-64 glass-panel rounded-2xl flex flex-col min-h-0">
      <div className="px-3 py-2.5 border-b border-border">
        <h3 className="text-xs font-medium text-muted-foreground tracking-wide">Version History</h3>
      </div>

      <div className="flex-1 overflow-y-auto">
        {versions.length === 0 && (
          <div className="px-3 py-8 text-center">
            <div className="text-dim text-xs">No versions yet</div>
            <div className="text-dim/60 text-2xs mt-1">
              Versions are created automatically as you edit
            </div>
          </div>
        )}
        {versions.map((v, i) => (
          <button
            key={v.id}
            type="button"
            onClick={() => setPreviewId(v.id === previewId ? null : v.id)}
            className={`w-full flex gap-3 px-3 py-2.5 text-left transition-colors ${
              v.id === previewId ? "bg-accent" : "hover:bg-card"
            }`}
          >
            {/* Timeline dot */}
            <div className="flex flex-col items-center pt-1.5">
              <div className={`version-dot ${v.id === previewId ? "version-dot-active" : ""}`} />
              {i < versions.length - 1 && <div className="version-line flex-1 min-h-[16px]" />}
            </div>
            {/* Content */}
            <div className="flex-1 min-w-0">
              <div
                className={`text-xs ${v.id === previewId ? "text-foreground font-medium" : "font-light text-muted-foreground"}`}
              >
                {formatRelativeTime(v.createdAt)}
              </div>
              <div className="text-2xs text-dim mt-0.5 truncate">
                {v.body.slice(0, 80)}
                {v.body.length > 80 ? "..." : ""}
              </div>
            </div>
          </button>
        ))}
      </div>

      {/* Preview + Restore */}
      {previewVersion && (
        <div className="border-t border-border px-3 py-2">
          <div className="text-2xs text-dim mb-1.5 max-h-20 overflow-y-auto whitespace-pre-wrap">
            {previewVersion.body.slice(0, 500)}
          </div>
          <button
            type="button"
            onClick={() => handleRestore(previewVersion.id)}
            disabled={restoring}
            className="flex items-center gap-1.5 px-2 py-1 rounded-md text-[11px] font-light text-brand hover:bg-brand/10 transition-colors disabled:opacity-50"
          >
            <RotateCcw className="size-3" strokeWidth={1.5} />
            {restoring ? "Restoring..." : "Restore this version"}
          </button>
          {restoreError && <p className="text-2xs text-destructive mt-1">{restoreError}</p>}
        </div>
      )}
    </div>
  );
}
