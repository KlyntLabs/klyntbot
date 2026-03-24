import { formatRelativeTime } from "@shared/lib/dates";
import { tagBgColor, tagColor } from "@shared/lib/tagColor";
import type { Note } from "@shared/types";
import { ChevronDown, ChevronRight, ExternalLink } from "lucide-react";
import { useMemo, useState } from "react";
import type { InsightReviewActions, InsightReviewState } from "../hooks/useInsightReview";
import { AISuggestionsPanel } from "./AISuggestionsPanel";
import { BacklinksPanel } from "./BacklinksPanel";
import { EntityReferencesPanel } from "./EntityReferencesPanel";
import { GraphMinimap } from "./GraphMinimap";
import { InsightReviewPanel } from "./InsightReviewPanel";

interface ContextPanelProps {
  width: number;
  noteId: string | null;
  isGraphMode: boolean;
  note: Note | null;
  notes: Note[];
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
      headings.push({ level: Number.parseInt(el.tagName[1]), text, index: i });
    }
  });
  return headings;
}

function scrollToHeading(text: string, index: number) {
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
  1: { size: "text-[11px]", weight: "font-medium", opacity: "opacity-90" },
  2: { size: "text-[10.5px]", weight: "font-normal", opacity: "opacity-70" },
  3: { size: "text-2xs", weight: "font-normal", opacity: "opacity-55" },
};

function TableOfContents({ bodyHtml }: { bodyHtml?: string | null }) {
  const headings = useMemo(() => parseHeadings(bodyHtml), [bodyHtml]);

  if (headings.length === 0) {
    return null;
  }

  const minLevel = Math.min(...headings.map((h) => h.level));

  return (
    <div>
      <div className="text-2xs font-medium text-dim uppercase tracking-wider mb-2">
        Table of Contents
      </div>
      <nav className="relative border-l border-border-subtle ml-1">
        {headings.map((h) => {
          const style = TOC_STYLES[h.level] || TOC_STYLES[3];
          const depth = h.level - minLevel;
          return (
            <button
              key={`${h.index}-${h.text}`}
              type="button"
              onClick={() => scrollToHeading(h.text, h.index)}
              className={`group flex items-center gap-1.5 w-full text-left ${style.size} ${style.weight} text-muted-foreground truncate py-[3px] pr-1 transition-all duration-150 hover:text-foreground hover:bg-white/[0.03] rounded-r-md`}
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
    <div className="border-b border-border">
      <button
        type="button"
        onClick={() => setCollapsed(!collapsed)}
        className="w-full flex items-center gap-1.5 px-3 py-2 text-2xs font-medium uppercase tracking-wider text-muted-foreground hover:text-foreground transition-colors"
      >
        {collapsed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
        <span>More</span>
      </button>

      {!collapsed && (
        <div className="px-3 pb-3 space-y-3">
          {/* Table of Contents */}
          <TableOfContents bodyHtml={note.bodyHtml} />

          {/* Note Metadata */}
          <div className="border-t border-border-subtle pt-2">
            <div className="text-2xs font-medium text-dim uppercase tracking-wider mb-1.5">
              Metadata
            </div>
            <div className="space-y-1">
              <div className="flex justify-between text-2xs">
                <span className="text-dim">Created</span>
                <span className="text-muted-foreground">{formatRelativeTime(note.createdAt)}</span>
              </div>
              <div className="flex justify-between text-2xs">
                <span className="text-dim">Updated</span>
                <span className="text-muted-foreground">{formatRelativeTime(note.updatedAt)}</span>
              </div>
              <div className="flex justify-between text-2xs">
                <span className="text-dim">Words</span>
                <span className="text-muted-foreground">{wordCount.toLocaleString()}</span>
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
        className="flex items-center gap-1.5 text-[11px] text-brand hover:text-foreground transition-colors"
      >
        <ExternalLink size={11} />
        Open in editor
      </button>

      <h2 className="text-[15px] font-medium text-foreground">{note.title}</h2>

      {note.tags.length > 0 && (
        <div className="flex gap-1 flex-wrap">
          {note.tags.map((tag) => (
            <span
              key={tag}
              className="text-2xs px-1.5 py-0.5 rounded"
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
        <div className="text-xs text-muted-foreground leading-relaxed whitespace-pre-wrap line-clamp-[20]">
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
  insightOpen,
  insightState,
  insightActions,
  onOpenInsight,
}: ContextPanelProps) {
  if (!noteId || !note) {
    return <div style={{ width }} className="glass-sidebar flex flex-col flex-shrink-0 h-full" />;
  }

  // Graph mode: show note preview instead of context sections
  if (isGraphMode) {
    return (
      <div
        style={{ width }}
        className="glass-sidebar flex flex-col flex-shrink-0 h-full overflow-y-auto"
      >
        <NotePreview note={note} onSelectNote={onSelectNote} />
      </div>
    );
  }

  // Editor mode: insight panel takes over when open
  if (insightOpen && insightState && insightActions) {
    return (
      <div style={{ width }} className="glass-sidebar flex flex-col flex-shrink-0 h-full">
        <InsightReviewPanel state={insightState} actions={insightActions} />
      </div>
    );
  }

  // Editor mode: show all context sections
  return (
    <div
      style={{ width }}
      className="glass-sidebar flex flex-col flex-shrink-0 h-full overflow-y-auto"
    >
      <AISuggestionsPanel
        noteId={noteId}
        perspectiveConfig={note.perspectiveConfig}
        onSelectNote={onSelectNote}
        onOpenInsight={onOpenInsight}
      />
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
