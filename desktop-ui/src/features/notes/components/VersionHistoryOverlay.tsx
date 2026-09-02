import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import { formatRelativeTime } from "@shared/lib/dates";
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
  const [restoreError, setRestoreError] = useState<string | null>(null);

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
    setRestoreError(null);
    try {
      const restored = await ipc<Note>("note_version_restore", {
        versionId: selectedId,
        noteId,
      });
      onRestore(restored);
      refetch();
    } catch (e) {
      console.error("Failed to restore version:", e);
      setRestoreError(e instanceof Error ? e.message : "Failed to restore version");
    } finally {
      setRestoring(false);
    }
  }, [selectedId, noteId, onRestore, refetch]);

  const diffResult = useMemo(() => {
    if (!selectedVersion || !showDiff) return null;
    return diffLines(selectedVersion.body, currentBody);
  }, [selectedVersion, currentBody, showDiff]);

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
        <div className="w-[40%] flex flex-col border-r border-separator">
          {/* Header */}
          <div className="shrink-0 flex items-center justify-between px-5 py-4 border-b border-separator">
            <h2 className="text-sm font-medium text-fg">Version History</h2>
            <button
              type="button"
              onClick={onClose}
              className="p-1.5 rounded-md text-fg-dim hover:text-fg transition-colors"
              aria-label="Close"
            >
              <X size={16} />
            </button>
          </div>

          {/* Timeline list */}
          <div className="flex-1 overflow-y-auto">
            {versions.length === 0 ? (
              <div className="px-5 py-12 text-center">
                <div className="text-fg-dim text-sm">No versions yet</div>
                <div className="text-fg-dim/60 text-ui-sm mt-1">
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
                      v.id === selectedId ? "bg-control-hover" : "hover:bg-bg-elevated"
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
                        <div className="w-px flex-1 min-h-[20px] bg-control-hover mt-1" />
                      )}
                    </div>

                    {/* Content */}
                    <div className="flex-1 min-w-0">
                      <div
                        className={`text-sm ${
                          v.id === selectedId
                            ? "text-fg font-medium"
                            : "text-fg-secondary"
                        }`}
                      >
                        {formatRelativeTime(v.createdAt)}
                      </div>
                      <div className="text-ui-sm text-fg-secondary mt-0.5">
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
          <div className="shrink-0 flex items-center justify-between px-5 py-4 border-b border-separator">
            <div className="flex items-center gap-3">
              {selectedVersion && (
                <>
                  <button
                    type="button"
                    onClick={handleRestore}
                    disabled={restoring}
                    className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm font-medium bg-brand text-white hover:bg-brand/90 transition-colors disabled:opacity-50"
                  >
                    <RotateCcw size={14} />
                    {restoring ? "Restoring..." : "Restore this version"}
                  </button>
                  {restoreError && (
                    <span className="text-ui-sm text-status-danger ml-2">{restoreError}</span>
                  )}
                </>
              )}
            </div>
            <div className="flex items-center gap-2">
              {selectedVersion && (
                <button
                  type="button"
                  onClick={() => setShowDiff((prev) => !prev)}
                  className={`px-3 py-1.5 rounded-lg text-ui-sm font-medium transition-colors ${
                    showDiff
                      ? "bg-control-hover text-fg"
                      : "text-fg-secondary hover:text-fg hover:bg-control-hover"
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
              <div className="flex items-center justify-center h-full text-fg-dim text-sm">
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
                          ? "bg-status-success/20 text-status-success"
                          : part.removed
                            ? "bg-status-danger/20 text-status-danger"
                            : "text-fg-secondary"
                      }
                    >
                      {part.value}
                    </span>
                  );
                })}
              </div>
            ) : (
              <div className="text-sm text-fg-secondary leading-relaxed whitespace-pre-wrap">
                {selectedVersion.body}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
