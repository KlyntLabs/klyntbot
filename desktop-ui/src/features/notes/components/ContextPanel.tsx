import { formatRelativeTime } from "@shared/lib/dates";
import { tagBgColor, tagColor } from "@shared/lib/tagColor";
import type { Note, NoteListItem } from "@shared/types";
import { ChevronDown, ChevronRight, ExternalLink } from "lucide-react";
import { useMemo, useState } from "react";
import type { InsightReviewActions, InsightReviewState } from "../hooks/useInsightReview";
import { AISuggestionsPanel } from "./AISuggestionsPanel";
import { BacklinksPanel } from "./BacklinksPanel";
import { EntityReferencesPanel } from "./EntityReferencesPanel";
import { InsightReviewPanel } from "./InsightReviewPanel";

interface ContextPanelProps {
  noteId: string | null;
  isGraphMode: boolean;
  note: Note | null;
  notes: NoteListItem[];
  onSelectNote: (id: string) => void;
  onExpandGraph: () => void;
  // Insight Review
  insightOpen?: boolean;
  insightState?: InsightReviewState;
  insightActions?: InsightReviewActions;
  onOpenInsight?: () => void;
}

// ── Table of Contents ─────────────────────────────────────────────────────

interface TocHeading {
  level: number;
  text: string;
  index: number;
}

function parseHeadings(html: string | null | undefined): TocHeading[] {
  if (!html) return [];
  const parser = new DOMParser();
  const doc = parser.parseFromString(html, "text/html");
  const headings: TocHeading[] = [];
  const els = doc.querySelectorAll("h1, h2, h3");
  els.forEach((el, i) => {
    const text = (el.textContent || "").trim();
    if (text) {
      headings.push({ level: Number.parseInt(el.tagName[1], 10), text, index: i });
    }
  });
  return headings;
}

function scrollToHeading(_text: string, index: number) {
  const editorEl = document.querySelector(".editor-content");
  if (!editorEl) return;
  const headings = editorEl.querySelectorAll("h1, h2, h3");
  // Match by index among all headings in the live editor DOM
  const target = headings[index];
  if (target) {
    target.scrollIntoView({ behavior: "smooth", block: "start" });
  }
}

const TOC_STYLES: Record<number, { size: string; weight: string; opacity: string }> = {
  1: { size: "text-ui-xs", weight: "font-medium", opacity: "opacity-90" },
  2: { size: "text-[10.5px]", weight: "font-normal", opacity: "opacity-70" },
  3: { size: "text-ui-xs", weight: "font-normal", opacity: "opacity-55" },
};

function TableOfContents({ bodyHtml }: { bodyHtml?: string | null }) {
  const headings = useMemo(() => parseHeadings(bodyHtml), [bodyHtml]);

  if (headings.length === 0) {
    return null;
  }

  const minLevel = Math.min(...headings.map((h) => h.level));

  return (
    <div>
      <div className="text-ui-xs font-medium text-fg-dim uppercase tracking-wider mb-2">
        Table of Contents
      </div>
      <nav className="relative border-l border-separator ml-1">
        {headings.map((h) => {
          const style = TOC_STYLES[h.level] || TOC_STYLES[3];
          const depth = h.level - minLevel;
          return (
            <button
              key={`${h.index}-${h.text}`}
              type="button"
              onClick={() => scrollToHeading(h.text, h.index)}
              className={`group flex items-center gap-1.5 w-full text-left ${style.size} ${style.weight} text-fg-secondary truncate py-[3px] pr-1 transition-all duration-150 hover:text-fg hover:bg-white/[0.03] rounded-r-md`}
              style={{ paddingLeft: `${8 + depth * 10}px` }}
              title={h.text}
            >
              <span
                className={`shrink-0 rounded-full bg-brand transition-opacity duration-150 ${style.opacity} group-hover:opacity-100`}
                style={{
                  width: h.level === 1 ? 4 : 3,
                  height: h.level === 1 ? 4 : 3,
                }}
              />
              <span className="truncate">{h.text}</span>
            </button>
          );
        })}
      </nav>
    </div>
  );
}

// ── More section (collapsed by default) ──────────────────────────────────

function MoreSection({ note }: { note: Note }) {
  const [collapsed, setCollapsed] = useState(true);

  const wordCount = useMemo(() => {
    if (!note.body) return 0;
    return note.body.trim().split(/\s+/).filter(Boolean).length;
  }, [note.body]);

  return (
    <div className="border-b border-separator">
      <button
        type="button"
        onClick={() => setCollapsed(!collapsed)}
        className="w-full flex items-center gap-1.5 px-3 py-2 text-ui-xs font-medium uppercase tracking-wider text-fg-secondary hover:text-fg transition-colors"
      >
        {collapsed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
        <span>More</span>
      </button>

      {!collapsed && (
        <div className="px-3 pb-3 space-y-3">
          {/* Table of Contents */}
          <TableOfContents bodyHtml={note.bodyHtml} />

          {/* Note Metadata */}
          <div className="border-t border-separator pt-2">
            <div className="text-ui-xs font-medium text-fg-dim uppercase tracking-wider mb-1.5">
              Metadata
            </div>
            <div className="space-y-1">
              <div className="flex justify-between text-ui-xs">
                <span className="text-fg-dim">Created</span>
                <span className="text-fg-secondary">{formatRelativeTime(note.createdAt)}</span>
              </div>
              <div className="flex justify-between text-ui-xs">
                <span className="text-fg-dim">Updated</span>
                <span className="text-fg-secondary">{formatRelativeTime(note.updatedAt)}</span>
              </div>
              <div className="flex justify-between text-ui-xs">
                <span className="text-fg-dim">Words</span>
                <span className="text-fg-secondary">{wordCount.toLocaleString()}</span>
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
        className="flex items-center gap-1.5 text-ui-xs text-brand hover:text-fg transition-colors"
      >
        <ExternalLink size={11} />
        Open in editor
      </button>

      <h2 className="text-[15px] font-medium text-fg">{note.title}</h2>

      {note.tags.length > 0 && (
        <div className="flex gap-1 flex-wrap">
          {note.tags.map((tag) => (
            <span
              key={tag}
              className="text-ui-xs px-1.5 py-0.5 rounded"
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
        <div className="text-ui-sm text-fg-secondary leading-relaxed whitespace-pre-wrap line-clamp-[20]">
          {note.body}
        </div>
      )}
    </div>
  );
}

// ── Main component ───────────────────────────────────────────────────────

export function ContextPanel({
  noteId,
  isGraphMode,
  note,
  notes,
  onSelectNote,
  onExpandGraph,
  insightOpen,
  insightState,
  insightActions,
  onOpenInsight,
}: ContextPanelProps) {
  if (!noteId || !note) {
    return <div className="glass-sidebar flex flex-col flex-shrink-0 h-full w-full" />;
  }

  // Graph mode: show note preview instead of context sections
  if (isGraphMode) {
    return (
      <div className="glass-sidebar flex flex-col flex-shrink-0 h-full w-full overflow-y-auto">
        <NotePreview note={note} onSelectNote={onSelectNote} />
      </div>
    );
  }

  // Editor mode: insight panel takes over when open
  if (insightOpen && insightState && insightActions) {
    return (
      <div className="glass-sidebar flex flex-col flex-shrink-0 h-full w-full">
        <InsightReviewPanel state={insightState} actions={insightActions} />
      </div>
    );
  }

  // Editor mode: show all context sections
  return (
    <div className="glass-sidebar flex flex-col flex-shrink-0 h-full w-full overflow-y-auto">
      <AISuggestionsPanel
        noteId={noteId}
        perspectiveConfig={note.perspectiveConfig}
        onSelectNote={onSelectNote}
        onOpenInsight={onOpenInsight}
      />
      <BacklinksPanel noteId={noteId} onSelectNote={onSelectNote} />
      <EntityReferencesPanel noteBody={note.body} />
      <MoreSection note={note} />
    </div>
  );
}
