import { formatRelativeTime } from "@shared/lib/dates";
import { tagBgColor, tagColor } from "@shared/lib/tagColor";
import type { Note } from "@shared/types";
import { ChevronDown, ChevronRight, ExternalLink } from "lucide-react";
import { useMemo, useState } from "react";
import { AISuggestionsPanel } from "./AISuggestionsPanel";
import { BacklinksPanel } from "./BacklinksPanel";
import { EntityReferencesPanel } from "./EntityReferencesPanel";
import { GraphMinimap } from "./GraphMinimap";

interface ContextPanelProps {
  width: number;
  noteId: string | null;
  isGraphMode: boolean;
  note: Note | null;
  notes: Note[];
  onSelectNote: (id: string) => void;
  onExpandGraph: () => void;
}

// ── More section (collapsed by default) ──────────────────────────────────

function MoreSection({ note }: { note: Note }) {
  const [collapsed, setCollapsed] = useState(true);

  const wordCount = useMemo(() => {
    if (!note.body) return 0;
    return note.body.trim().split(/\s+/).filter(Boolean).length;
  }, [note.body]);

  return (
    <div className="border-b border-border">
      <button
        type="button"
        onClick={() => setCollapsed(!collapsed)}
        className="w-full flex items-center gap-1.5 px-3 py-2 text-[10px] font-medium uppercase tracking-wider text-muted hover:text-secondary transition-colors"
      >
        {collapsed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
        <span>More</span>
      </button>

      {!collapsed && (
        <div className="px-3 pb-3 space-y-3">
          {/* Table of Contents placeholder */}
          <div>
            <div className="text-[10px] font-medium text-dim uppercase tracking-wider mb-1">
              Table of Contents
            </div>
            <div className="text-[10px] text-dim italic">Table of contents coming soon</div>
          </div>

          {/* Note Metadata */}
          <div className="border-t border-white/[0.04] pt-2">
            <div className="text-[10px] font-medium text-dim uppercase tracking-wider mb-1.5">
              Metadata
            </div>
            <div className="space-y-1">
              <div className="flex justify-between text-[10px]">
                <span className="text-dim">Created</span>
                <span className="text-muted">{formatRelativeTime(note.createdAt)}</span>
              </div>
              <div className="flex justify-between text-[10px]">
                <span className="text-dim">Updated</span>
                <span className="text-muted">{formatRelativeTime(note.updatedAt)}</span>
              </div>
              <div className="flex justify-between text-[10px]">
                <span className="text-dim">Words</span>
                <span className="text-muted">{wordCount.toLocaleString()}</span>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

// ── Graph mode preview ───────────────────────────────────────────────────

function NotePreview({ note, onSelectNote }: { note: Note; onSelectNote: (id: string) => void }) {
  return (
    <div className="p-3 space-y-3">
      <button
        type="button"
        onClick={() => onSelectNote(note.id)}
        className="flex items-center gap-1.5 text-[11px] text-brand hover:text-primary transition-colors"
      >
        <ExternalLink size={11} />
        Open in editor
      </button>

      <h2 className="text-[15px] font-medium text-primary">{note.title}</h2>

      {note.tags.length > 0 && (
        <div className="flex gap-1 flex-wrap">
          {note.tags.map((tag) => (
            <span
              key={tag}
              className="text-[10px] px-1.5 py-0.5 rounded"
              style={{
                color: tagColor(tag),
                backgroundColor: tagBgColor(tag),
              }}
            >
              {tag}
            </span>
          ))}
        </div>
      )}

      {note.body && (
        <div className="text-[12px] text-secondary leading-relaxed whitespace-pre-wrap line-clamp-[20]">
          {note.body}
        </div>
      )}
    </div>
  );
}

// ── Main component ───────────────────────────────────────────────────────

export function ContextPanel({
  width,
  noteId,
  isGraphMode,
  note,
  notes,
  onSelectNote,
  onExpandGraph,
}: ContextPanelProps) {
  if (!noteId || !note) {
    return (
      <div
        style={{ width }}
        className="border-l border-border flex flex-col flex-shrink-0 h-full bg-white/[0.02]"
      />
    );
  }

  // Graph mode: show note preview instead of context sections
  if (isGraphMode) {
    return (
      <div
        style={{ width }}
        className="border-l border-border flex flex-col flex-shrink-0 h-full overflow-y-auto bg-white/[0.02]"
      >
        <NotePreview note={note} onSelectNote={onSelectNote} />
      </div>
    );
  }

  // Editor mode: show all context sections
  return (
    <div
      style={{ width }}
      className="border-l border-border flex flex-col flex-shrink-0 h-full overflow-y-auto bg-white/[0.02]"
    >
      <AISuggestionsPanel noteId={noteId} onSelectNote={onSelectNote} />
      <BacklinksPanel noteId={noteId} onSelectNote={onSelectNote} />
      <EntityReferencesPanel noteBody={note.body} />
      <GraphMinimap
        noteId={noteId}
        notes={notes}
        onSelectNote={onSelectNote}
        onExpandGraph={onExpandGraph}
      />
      <MoreSection note={note} />
    </div>
  );
}
