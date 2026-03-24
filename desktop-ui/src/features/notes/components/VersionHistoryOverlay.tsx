import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import type { Note, NoteVersion } from "@shared/types";
import { diffLines } from "diff";
import { RotateCcw, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";

interface VersionHistoryOverlayProps {
  noteId: string;
  currentBody: string;
  onClose: () => void;
  onRestore: (restored: Note) => void;
}

export function VersionHistoryOverlay({
  noteId,
  currentBody,
  onClose,
  onRestore,
}: VersionHistoryOverlayProps) {
  const { data: versions, refetch } = useQuery<NoteVersion[]>("note_version_list", { noteId }, []);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [showDiff, setShowDiff] = useState(false);
  const [restoring, setRestoring] = useState(false);

  // Auto-select first version
  useEffect(() => {
    if (versions.length > 0 && !selectedId) {
      setSelectedId(versions[0].id);
    }
  }, [versions, selectedId]);

  const selectedVersion = useMemo(
    () => versions.find((v) => v.id === selectedId),
    [versions, selectedId],
  );

  // Escape to close
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [onClose]);

  const handleRestore = useCallback(async () => {
    if (!selectedId) return;
    setRestoring(true);
    try {
      const restored = await ipc<Note>("note_version_restore", {
        versionId: selectedId,
        noteId,
      });
      onRestore(restored);
      refetch();
    } catch (e) {
      console.error("Failed to restore version:", e);
    } finally {
      setRestoring(false);
    }
  }, [selectedId, noteId, onRestore, refetch]);

  const diffResult = useMemo(() => {
    if (!selectedVersion || !showDiff) return null;
    return diffLines(selectedVersion.body, currentBody);
  }, [selectedVersion, currentBody, showDiff]);

  const formatTime = (iso: string) => {
    const d = new Date(iso);
    const now = new Date();
    const diffMs = now.getTime() - d.getTime();
    const diffMin = Math.floor(diffMs / 60000);
    if (diffMin < 1) return "just now";
    if (diffMin < 60) return `${diffMin} min ago`;
    const diffHr = Math.floor(diffMin / 60);
    if (diffHr < 24) return `${diffHr} hour${diffHr !== 1 ? "s" : ""} ago`;
    const diffDay = Math.floor(diffHr / 24);
    if (diffDay < 7) return `${diffDay} day${diffDay !== 1 ? "s" : ""} ago`;
    return d.toLocaleDateString();
  };

  const wordCountDelta = (version: NoteVersion) => {
    const versionWords = version.body.split(/\s+/).filter(Boolean).length;
    const currentWords = currentBody.split(/\s+/).filter(Boolean).length;
    const delta = currentWords - versionWords;
    if (delta > 0) return `+${delta} words`;
    if (delta < 0) return `${delta} words`;
    return "no change";
  };

  return (
    <div className="fixed inset-0 z-50 flex">
      {/* Backdrop */}
      <div className="absolute inset-0 glass-panel" />

      {/* Content */}
      <div className="relative flex w-full h-full">
        {/* Left side: Timeline (40%) */}
        <div className="w-[40%] flex flex-col border-r border-border-subtle">
          {/* Header */}
          <div className="shrink-0 flex items-center justify-between px-5 py-4 border-b border-border-subtle">
            <h2 className="text-sm font-medium text-foreground">Version History</h2>
            <button
              type="button"
              onClick={onClose}
              className="p-1.5 rounded-md text-dim hover:text-foreground transition-colors"
              aria-label="Close"
            >
              <X size={16} />
            </button>
          </div>

          {/* Timeline list */}
          <div className="flex-1 overflow-y-auto">
            {versions.length === 0 ? (
              <div className="px-5 py-12 text-center">
                <div className="text-dim text-sm">No versions yet</div>
                <div className="text-dim/60 text-xs mt-1">
                  Versions are created automatically as you edit
                </div>
              </div>
            ) : (
              <div className="py-2">
                {versions.map((v, i) => (
                  <button
                    key={v.id}
                    type="button"
                    onClick={() => setSelectedId(v.id)}
                    className={`w-full flex gap-3 px-5 py-3 text-left transition-colors ${
                      v.id === selectedId ? "bg-muted" : "hover:bg-card"
                    }`}
                  >
                    {/* Timeline dot + line */}
                    <div className="flex flex-col items-center pt-1.5">
                      <div
                        className={`size-2.5 rounded-full border-2 transition-colors ${
                          v.id === selectedId
                            ? "border-brand bg-brand"
                            : "border-dim bg-transparent"
                        }`}
                      />
                      {i < versions.length - 1 && (
                        <div className="w-px flex-1 min-h-[20px] bg-muted mt-1" />
                      )}
                    </div>

                    {/* Content */}
                    <div className="flex-1 min-w-0">
                      <div
                        className={`text-sm ${
                          v.id === selectedId
                            ? "text-foreground font-medium"
                            : "text-muted-foreground"
                        }`}
                      >
                        {formatTime(v.createdAt)}
                      </div>
                      <div className="text-xs text-muted-foreground mt-0.5">
                        {wordCountDelta(v)}
                      </div>
                    </div>
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Right side: Preview (60%) */}
        <div className="w-[60%] flex flex-col">
          {/* Header with actions */}
          <div className="shrink-0 flex items-center justify-between px-5 py-4 border-b border-border-subtle">
            <div className="flex items-center gap-3">
              {selectedVersion && (
                <button
                  type="button"
                  onClick={handleRestore}
                  disabled={restoring}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm font-medium bg-brand text-white hover:bg-brand/90 transition-colors disabled:opacity-50"
                >
                  <RotateCcw size={14} />
                  {restoring ? "Restoring..." : "Restore this version"}
                </button>
              )}
            </div>
            <div className="flex items-center gap-2">
              {selectedVersion && (
                <button
                  type="button"
                  onClick={() => setShowDiff((prev) => !prev)}
                  className={`px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
                    showDiff
                      ? "bg-muted text-foreground"
                      : "text-muted-foreground hover:text-foreground hover:bg-accent"
                  }`}
                >
                  {showDiff ? "Hide diff" : "Show diff"}
                </button>
              )}
            </div>
          </div>

          {/* Preview content */}
          <div className="flex-1 overflow-y-auto px-8 py-6">
            {!selectedVersion ? (
              <div className="flex items-center justify-center h-full text-dim text-sm">
                Select a version to preview
              </div>
            ) : showDiff && diffResult ? (
              <div className="font-mono text-sm leading-relaxed whitespace-pre-wrap">
                {diffResult.map((part, idx) => {
                  const key = `${idx}${part.added ? "a" : ""}${part.removed ? "r" : ""}`;
                  return (
                    <span
                      key={key}
                      className={
                        part.added
                          ? "bg-success/20 text-success"
                          : part.removed
                            ? "bg-destructive/20 text-destructive"
                            : "text-muted-foreground"
                      }
                    >
                      {part.value}
                    </span>
                  );
                })}
              </div>
            ) : (
              <div className="text-sm text-muted-foreground leading-relaxed whitespace-pre-wrap">
                {selectedVersion.body}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
