import { ipc } from "@shared/hooks/useIpc";
import type { Note } from "@shared/types";
import { FileText, Sparkles, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

interface NoteCreationDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onCreate: (title: string) => Promise<{ id: string } | undefined>;
  onNavigateNote: (id: string) => void;
}

export function NoteCreationDialog({
  isOpen,
  onClose,
  onCreate,
  onNavigateNote,
}: NoteCreationDialogProps) {
  const [title, setTitle] = useState("");
  const [similarNotes, setSimilarNotes] = useState<Note[]>([]);
  const [searching, setSearching] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Auto-focus input when dialog opens
  useEffect(() => {
    if (isOpen) {
      setTitle("");
      setSimilarNotes([]);
      // Small delay to ensure the dialog is rendered
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [isOpen]);

  // Debounced search for similar notes
  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    const q = title.trim();
    if (q.length < 3) {
      setSimilarNotes([]);
      setSearching(false);
      return;
    }
    setSearching(true);
    debounceRef.current = setTimeout(async () => {
      try {
        const results = await ipc<Note[]>("note_search", { query: q });
        setSimilarNotes(results.slice(0, 5));
      } catch {
        setSimilarNotes([]);
      } finally {
        setSearching(false);
      }
    }, 200);
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [title]);

  const handleCreate = useCallback(async () => {
    const t = title.trim();
    if (!t) return;
    const result = await onCreate(t);
    if (result) {
      onNavigateNote(result.id);
      onClose();
    }
  }, [title, onCreate, onNavigateNote, onClose]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleCreate();
      } else if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    },
    [handleCreate, onClose],
  );

  const handleNavigate = useCallback(
    (id: string) => {
      onNavigateNote(id);
      onClose();
    },
    [onNavigateNote, onClose],
  );

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      {/* biome-ignore lint/a11y/useKeyWithClickEvents: backdrop dismiss pattern */}
      {/* biome-ignore lint/a11y/noStaticElementInteractions: backdrop dismiss */}
      <div className="absolute inset-0 bg-black/50" onClick={onClose} />

      {/* Dialog */}
      <div className="relative w-full max-w-[480px] mx-4 glass-panel rounded-2xl border border-white/[0.06] shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between px-5 pt-4 pb-2">
          <h2 className="text-sm font-medium text-primary">New Note</h2>
          <button
            type="button"
            onClick={onClose}
            className="p-1 rounded-md text-dim hover:text-secondary transition-colors"
            aria-label="Close"
          >
            <X size={16} />
          </button>
        </div>

        {/* Title input */}
        <div className="px-5 pb-3">
          <input
            ref={inputRef}
            type="text"
            placeholder="Note title..."
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            onKeyDown={handleKeyDown}
            className="w-full text-lg bg-transparent border-none outline-none text-primary placeholder:text-dim/50 font-medium"
          />
        </div>

        {/* Similar notes */}
        {(similarNotes.length > 0 || searching) && (
          <div className="border-t border-white/[0.06] px-5 py-3">
            <div className="flex items-center gap-1.5 mb-2">
              <Sparkles size={12} className="text-brand" />
              <span className="text-[10px] uppercase tracking-wider text-muted">Similar notes</span>
            </div>
            {searching && similarNotes.length === 0 ? (
              <div className="text-xs text-dim py-1">Searching...</div>
            ) : (
              <div className="flex flex-col gap-0.5">
                {similarNotes.map((note) => (
                  <button
                    key={note.id}
                    type="button"
                    onClick={() => handleNavigate(note.id)}
                    className="flex items-center gap-2 px-2 py-1.5 rounded-lg text-left hover:bg-white/[0.06] transition-colors group"
                  >
                    <FileText size={14} className="text-dim group-hover:text-muted shrink-0" />
                    <span className="text-sm text-secondary group-hover:text-primary truncate">
                      {note.title || "Untitled"}
                    </span>
                  </button>
                ))}
              </div>
            )}
          </div>
        )}

        {/* Actions */}
        <div className="border-t border-white/[0.06] px-5 py-3 flex items-center justify-between">
          <button
            type="button"
            onClick={handleCreate}
            className="text-xs text-dim hover:text-muted transition-colors"
          >
            Create blank
          </button>
          <button
            type="button"
            onClick={handleCreate}
            disabled={!title.trim()}
            className="px-4 py-1.5 rounded-lg text-sm font-medium bg-brand text-white hover:bg-brand/90 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            Create
          </button>
        </div>
      </div>
    </div>
  );
}
