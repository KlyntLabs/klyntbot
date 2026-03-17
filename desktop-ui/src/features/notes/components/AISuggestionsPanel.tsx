import { tagBgColor, tagColor } from "@shared/lib/tagColor";
import { Brain, ChevronDown, ChevronRight, Link2, MessageSquare } from "lucide-react";
import { useEffect, useState } from "react";
import { useNoteSuggestions } from "../hooks/useNoteSuggestions";

interface AISuggestionsPanelProps {
  noteId: string | null;
  onSelectNote: (id: string) => void;
  onOpenInsight?: () => void;
}

const ACCENT = "rgba(167, 139, 250, 0.85)";
const ACCENT_BG = "rgba(139, 92, 246, 0.08)";

function insertWikiLink(noteId: string, title: string) {
  window.dispatchEvent(new CustomEvent("insert-wiki-link", { detail: { noteId, title } }));
}

export function AISuggestionsPanel({
  noteId,
  onSelectNote,
  onOpenInsight,
}: AISuggestionsPanelProps) {
  const { suggestions } = useNoteSuggestions(noteId);
  const [collapsed, setCollapsed] = useState(false);
  const [showLinkPicker, setShowLinkPicker] = useState(false);

  // Cmd+L shortcut: insert top suggestion
  useEffect(() => {
    const handler = () => {
      const links = suggestions.linkSuggestions;
      if (links.length > 0) {
        insertWikiLink(links[0].note.id, links[0].note.title);
      }
    };
    window.addEventListener("trigger-insert-link", handler);
    return () => window.removeEventListener("trigger-insert-link", handler);
  }, [suggestions.linkSuggestions]);

  const hasData =
    suggestions.relatedNotes.length > 0 ||
    suggestions.linkSuggestions.length > 0 ||
    suggestions.suggestedTags.length > 0;

  return (
    <div className="border-b border-border" style={{ borderLeftColor: ACCENT, borderLeftWidth: 2 }}>
      <button
        type="button"
        onClick={() => setCollapsed(!collapsed)}
        className="w-full flex items-center gap-1.5 px-3 py-2 text-[10px] font-medium uppercase tracking-wider text-muted hover:text-secondary transition-colors"
      >
        {collapsed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
        <span style={{ color: ACCENT }}>AI Suggestions</span>
        {hasData && (
          <span className="ml-auto text-[9px] text-dim">
            {suggestions.relatedNotes.length + suggestions.linkSuggestions.length}
          </span>
        )}
      </button>

      {!collapsed && (
        <div className="px-3 pb-3" style={{ backgroundColor: ACCENT_BG }}>
          {/* Related Notes */}
          <div className="py-1.5">
            <div className="text-[10px] font-medium text-dim uppercase tracking-wider mb-1">
              Related Notes
            </div>
            {suggestions.relatedNotes.length > 0 ? (
              <div className="flex flex-col gap-0.5">
                {suggestions.relatedNotes.map((item) => (
                  <button
                    key={item.note.id}
                    type="button"
                    onClick={() => onSelectNote(item.note.id)}
                    className="flex flex-col gap-0.5 px-2 py-1 rounded text-left hover:bg-surface-base transition-colors"
                  >
                    <span className="text-[11px] text-secondary truncate">{item.note.title}</span>
                    <span className="text-[9px] text-dim truncate">{item.reason}</span>
                  </button>
                ))}
              </div>
            ) : (
              <div className="text-[10px] text-dim italic">
                {noteId ? "No suggestions yet" : "Select a note"}
              </div>
            )}
          </div>

          {/* Link Suggestions */}
          <div className="py-1.5 border-t border-border-subtle">
            <div className="text-[10px] font-medium text-dim uppercase tracking-wider mb-1">
              Link Suggestions
            </div>
            {suggestions.linkSuggestions.length > 0 ? (
              <div className="flex flex-col gap-0.5">
                {suggestions.linkSuggestions.map((item) => (
                  <button
                    key={item.note.id}
                    type="button"
                    onClick={() => onSelectNote(item.note.id)}
                    className="flex flex-col gap-0.5 px-2 py-1 rounded text-left hover:bg-surface-base transition-colors"
                  >
                    <span className="text-[11px] text-secondary truncate">
                      Link to: {item.note.title}
                    </span>
                    <span className="text-[9px] text-dim truncate">{item.reason}</span>
                  </button>
                ))}
              </div>
            ) : (
              <div className="text-[10px] text-dim italic">No link suggestions</div>
            )}
          </div>

          {/* Suggested Tags */}
          <div className="py-1.5 border-t border-border-subtle">
            <div className="text-[10px] font-medium text-dim uppercase tracking-wider mb-1">
              Suggested Tags
            </div>
            {suggestions.suggestedTags.length > 0 ? (
              <div className="flex flex-wrap gap-1">
                {suggestions.suggestedTags.map((tag) => (
                  <span
                    key={tag}
                    className="text-[10px] px-1.5 py-0.5 rounded-full"
                    style={{
                      color: tagColor(tag),
                      backgroundColor: tagBgColor(tag),
                    }}
                  >
                    +{tag}
                  </span>
                ))}
              </div>
            ) : (
              <div className="text-[10px] text-dim italic">No tag suggestions</div>
            )}
          </div>

          {/* Action buttons */}
          <div className="flex gap-1.5 mt-2 pt-2 border-t border-border-subtle relative">
            <button
              type="button"
              onClick={() => onOpenInsight?.()}
              disabled={!noteId}
              className="flex items-center gap-1 text-[10px] px-2 py-1 rounded-md bg-surface-low text-secondary hover:bg-surface-raised hover:text-primary transition-colors disabled:text-dim disabled:cursor-not-allowed"
            >
              <Brain size={10} />
              Insight Review
            </button>
            <button
              type="button"
              disabled
              className="flex items-center gap-1 text-[10px] px-2 py-1 rounded-md bg-surface-low text-dim cursor-not-allowed"
            >
              <MessageSquare size={10} />
              Ask AI
            </button>
            <button
              type="button"
              onClick={() => {
                const links = suggestions.linkSuggestions;
                if (links.length === 1) {
                  insertWikiLink(links[0].note.id, links[0].note.title);
                } else if (links.length > 1) {
                  setShowLinkPicker((v) => !v);
                }
              }}
              disabled={!noteId || suggestions.linkSuggestions.length === 0}
              className={`flex items-center gap-1 text-[10px] px-2 py-1 rounded-md transition-colors ${
                noteId && suggestions.linkSuggestions.length > 0
                  ? "bg-surface-base text-secondary hover:bg-surface-raised hover:text-primary"
                  : "bg-surface-low text-dim cursor-not-allowed"
              }`}
            >
              <Link2 size={10} />
              Insert link
            </button>

            {/* Link picker popover */}
            {showLinkPicker && (
              <div className="absolute bottom-full left-0 right-0 mb-1 glass-dropdown rounded-lg py-1 shadow-lg z-10">
                {suggestions.linkSuggestions.map((item) => (
                  <button
                    key={item.note.id}
                    type="button"
                    onClick={() => {
                      insertWikiLink(item.note.id, item.note.title);
                      setShowLinkPicker(false);
                    }}
                    className="w-full text-left px-3 py-1.5 text-[11px] text-secondary hover:bg-surface-base hover:text-primary transition-colors truncate"
                  >
                    [[{item.note.title}]]
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
