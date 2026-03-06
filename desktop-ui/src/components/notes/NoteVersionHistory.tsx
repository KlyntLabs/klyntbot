import { RotateCcw } from "lucide-react";
import { useCallback, useState } from "react";
import { ipc } from "../../hooks/useIpc";
import { useQuery } from "../../hooks/useQuery";
import type { Note, NoteVersion } from "../../lib/types";

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
      <div className="px-3 py-2 border-b border-border">
        <h3 className="text-xs font-medium text-secondary">Version History</h3>
      </div>

      <div className="flex-1 overflow-y-auto">
        {versions.length === 0 && (
          <div className="px-3 py-6 text-center">
            <p className="text-xs text-dim">No versions yet</p>
          </div>
        )}
        {versions.map((v) => (
          <button
            key={v.id}
            type="button"
            onClick={() => setPreviewId(v.id === previewId ? null : v.id)}
            className={`w-full px-3 py-2 text-left border-b border-white/[0.04] last:border-b-0 transition-colors ${
              v.id === previewId
                ? "bg-white/[0.08] text-primary"
                : "text-secondary hover:bg-white/[0.04]"
            }`}
          >
            <div className="text-xs font-light">{formatTime(v.createdAt)}</div>
            <div className="text-[10px] text-dim mt-0.5 truncate">
              {v.body.slice(0, 80)}
              {v.body.length > 80 ? "..." : ""}
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
