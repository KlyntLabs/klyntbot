import { RotateCcw } from "lucide-react";
import { useCallback, useState } from "react";
import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import type { Note, NoteVersion } from "@shared/types";

interface NoteVersionHistoryProps {
  noteId: string;
  onRestore: (restored: Note) => void;
}

export function NoteVersionHistory({ noteId, onRestore }: NoteVersionHistoryProps) {
  const { data: versions, refetch } = useQuery<NoteVersion[]>("note_version_list", { noteId }, []);
  const [previewId, setPreviewId] = useState<string | null>(null);
  const [restoring, setRestoring] = useState(false);

  const previewVersion = versions.find((v) => v.id === previewId);

  const handleRestore = useCallback(
    async (versionId: string) => {
      setRestoring(true);
      try {
        const restored = await ipc<Note>("note_version_restore", { versionId, noteId });
        onRestore(restored);
        refetch();
        setPreviewId(null);
      } catch (e) {
        console.error("Failed to restore version:", e);
      } finally {
        setRestoring(false);
      }
    },
    [noteId, onRestore, refetch],
  );

  const formatTime = (iso: string) => {
    const d = new Date(iso);
    const now = new Date();
    const diffMs = now.getTime() - d.getTime();
    const diffMin = Math.floor(diffMs / 60000);
    if (diffMin < 1) return "just now";
    if (diffMin < 60) return `${diffMin}m ago`;
    const diffHr = Math.floor(diffMin / 60);
    if (diffHr < 24) return `${diffHr}h ago`;
    const diffDay = Math.floor(diffHr / 24);
    if (diffDay < 7) return `${diffDay}d ago`;
    return d.toLocaleDateString();
  };

  return (
    <div className="w-64 glass-panel rounded-2xl flex flex-col min-h-0">
      <div className="px-3 py-2.5 border-b border-border">
        <h3 className="text-xs font-medium text-secondary tracking-wide">Version History</h3>
      </div>

      <div className="flex-1 overflow-y-auto">
        {versions.length === 0 && (
          <div className="px-3 py-8 text-center">
            <div className="text-dim text-xs">No versions yet</div>
            <div className="text-dim/60 text-[10px] mt-1">
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
              v.id === previewId ? "bg-white/[0.06]" : "hover:bg-white/[0.03]"
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
                className={`text-xs ${v.id === previewId ? "text-primary font-medium" : "font-light text-secondary"}`}
              >
                {formatTime(v.createdAt)}
              </div>
              <div className="text-[10px] text-dim mt-0.5 truncate">
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
          <div className="text-[10px] text-dim mb-1.5 max-h-20 overflow-y-auto whitespace-pre-wrap">
            {previewVersion.body.slice(0, 500)}
          </div>
          <button
            type="button"
            onClick={() => handleRestore(previewVersion.id)}
            disabled={restoring}
            className="flex items-center gap-1.5 px-2 py-1 rounded-md text-[11px] font-light text-brand hover:bg-brand/10 transition-colors disabled:opacity-50"
          >
            <RotateCcw className="w-3 h-3" strokeWidth={1.5} />
            {restoring ? "Restoring..." : "Restore this version"}
          </button>
        </div>
      )}
    </div>
  );
}
