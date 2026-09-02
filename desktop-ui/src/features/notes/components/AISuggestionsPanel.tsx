import { findLanguage, LANGUAGES } from "@shared/constants/languages";
import { ipc } from "@shared/hooks/useIpc";
import { tagBgColor, tagColor } from "@shared/lib/tagColor";
import { Brain, ChevronDown, ChevronRight, Link2, MessageSquare } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useLanguageConfig } from "../hooks/useLanguageConfig";
import { useNoteSuggestions } from "../hooks/useNoteSuggestions";

interface AISuggestionsPanelProps {
  noteId: string | null;
  perspectiveConfig: string | null;
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
  perspectiveConfig,
  onSelectNote,
  onOpenInsight,
}: AISuggestionsPanelProps) {
  const { suggestions } = useNoteSuggestions(noteId);
  const [collapsed, setCollapsed] = useState(false);
  const [showLinkPicker, setShowLinkPicker] = useState(false);

  const { sourceLang, targetLang } = useLanguageConfig(perspectiveConfig);

  const handleLanguageChange = useCallback(
    (field: "sourceLang" | "targetLang", code: string) => {
      if (!noteId) return;
      const current = field === "sourceLang" ? sourceLang : targetLang;
      if (code === current) return;
      let config: Record<string, unknown> = {};
      if (perspectiveConfig) {
        try {
          config = JSON.parse(perspectiveConfig);
        } catch (e) {
          console.warn("Failed to parse perspectiveConfig:", e);
        }
      }
      const pair = (config.languagePair as Record<string, string>) ?? {};
      config.languagePair = { ...pair, [field]: code };
      ipc("note_update", {
        params: { id: noteId, perspectiveConfig: JSON.stringify(config) },
      }).catch((e) => console.error("Failed to update language config:", e));
    },
    [noteId, perspectiveConfig, sourceLang, targetLang],
  );

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
    <>
      <div className="border-b border-separator px-3 py-2">
        <div className="text-ui-xs font-medium text-fg-dim uppercase tracking-wider mb-1.5">
          Language
        </div>
        <div className="flex gap-2">
          <LanguageDropdown
            label="Source"
            value={sourceLang}
            onChange={(code) => handleLanguageChange("sourceLang", code)}
            disabled={!noteId}
          />
          <LanguageDropdown
            label="Target"
            value={targetLang}
            onChange={(code) => handleLanguageChange("targetLang", code)}
            disabled={!noteId}
          />
        </div>
      </div>

      {/* AI Suggestions section */}
      <div
        className="border-b border-separator"
        style={{ borderLeftColor: ACCENT, borderLeftWidth: 2 }}
      >
        <button
          type="button"
          onClick={() => setCollapsed(!collapsed)}
          className="w-full flex items-center gap-1.5 px-3 py-2 text-ui-xs font-medium uppercase tracking-wider text-fg-secondary hover:text-fg transition-colors"
        >
          {collapsed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
          <span style={{ color: ACCENT }}>AI Suggestions</span>
          {hasData && (
            <span className="ml-auto text-[9px] text-fg-dim">
              {suggestions.relatedNotes.length + suggestions.linkSuggestions.length}
            </span>
          )}
        </button>

        {!collapsed && (
          <div className="px-3 pb-3" style={{ backgroundColor: ACCENT_BG }}>
            {/* Related Notes */}
            <div className="py-1.5">
              <div className="text-ui-xs font-medium text-fg-dim uppercase tracking-wider mb-1">
                Related Notes
              </div>
              {suggestions.relatedNotes.length > 0 ? (
                <div className="flex flex-col gap-0.5">
                  {suggestions.relatedNotes.map((item) => (
                    <button
                      key={item.note.id}
                      type="button"
                      onClick={() => onSelectNote(item.note.id)}
                      className="flex flex-col gap-0.5 px-2 py-1 rounded text-left hover:bg-control-hover transition-colors"
                    >
                      <span className="text-ui-xs text-fg-secondary truncate">
                        {item.note.title}
                      </span>
                      <span className="text-[9px] text-fg-dim truncate">{item.reason}</span>
                    </button>
                  ))}
                </div>
              ) : (
                <div className="text-ui-xs text-fg-dim italic">
                  {noteId ? "No suggestions yet" : "Select a note"}
                </div>
              )}
            </div>

            {/* Link Suggestions */}
            <div className="py-1.5 border-t border-separator">
              <div className="text-ui-xs font-medium text-fg-dim uppercase tracking-wider mb-1">
                Link Suggestions
              </div>
              {suggestions.linkSuggestions.length > 0 ? (
                <div className="flex flex-col gap-0.5">
                  {suggestions.linkSuggestions.map((item) => (
                    <button
                      key={item.note.id}
                      type="button"
                      onClick={() => onSelectNote(item.note.id)}
                      className="flex flex-col gap-0.5 px-2 py-1 rounded text-left hover:bg-control-hover transition-colors"
                    >
                      <span className="text-ui-xs text-fg-secondary truncate">
                        Link to: {item.note.title}
                      </span>
                      <span className="text-[9px] text-fg-dim truncate">{item.reason}</span>
                    </button>
                  ))}
                </div>
              ) : (
                <div className="text-ui-xs text-fg-dim italic">No link suggestions</div>
              )}
            </div>

            {/* Suggested Tags */}
            <div className="py-1.5 border-t border-separator">
              <div className="text-ui-xs font-medium text-fg-dim uppercase tracking-wider mb-1">
                Suggested Tags
              </div>
              {suggestions.suggestedTags.length > 0 ? (
                <div className="flex flex-wrap gap-1">
                  {suggestions.suggestedTags.map((tag) => (
                    <span
                      key={tag}
                      className="text-ui-xs px-1.5 py-0.5 rounded-full"
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
                <div className="text-ui-xs text-fg-dim italic">No tag suggestions</div>
              )}
            </div>

            {/* Action buttons */}
            <div className="flex gap-1.5 mt-2 pt-2 border-t border-separator relative">
              <button
                type="button"
                onClick={() => onOpenInsight?.()}
                disabled={!noteId}
                className="flex items-center gap-1 text-ui-xs px-2 py-1 rounded-md bg-control-hover text-fg-secondary hover:bg-control-hover/80 hover:text-fg transition-colors disabled:text-fg-dim disabled:cursor-not-allowed"
              >
                <Brain size={10} />
                Learn
              </button>
              <button
                type="button"
                disabled
                className="flex items-center gap-1 text-ui-xs px-2 py-1 rounded-md bg-control-hover text-fg-dim cursor-not-allowed"
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
                className={`flex items-center gap-1 text-ui-xs px-2 py-1 rounded-md transition-colors ${
                  noteId && suggestions.linkSuggestions.length > 0
                    ? "bg-control-hover text-fg-secondary hover:bg-control-hover/80 hover:text-fg"
                    : "bg-control-hover text-fg-dim cursor-not-allowed"
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
                      className="w-full text-left px-3 py-1.5 text-ui-xs text-fg-secondary hover:bg-control-hover hover:text-fg transition-colors truncate"
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
    </>
  );
}

function LanguageDropdown({
  label,
  value,
  onChange,
  disabled,
}: {
  label: string;
  value: string;
  onChange: (code: string) => void;
  disabled: boolean;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const portalRef = useRef<HTMLDivElement>(null);
  const lang = findLanguage(value);
  const display = lang ? `${lang.flag} ${lang.native}` : value;

  // Click outside to close — checks both the trigger and the portal menu
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      const target = e.target as Node;
      const insideTrigger = ref.current?.contains(target) ?? false;
      const insidePortal = portalRef.current?.contains(target) ?? false;
      if (!insideTrigger && !insidePortal) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  return (
    <div className="flex-1" ref={ref}>
      <button
        type="button"
        disabled={disabled}
        onClick={() => setOpen(!open)}
        className="w-full flex items-center justify-between gap-1 px-2 py-1 rounded-md text-ui-xs text-fg-secondary bg-bg-elevated hover:bg-control-hover transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
      >
        <span className="truncate">{display}</span>
        <ChevronDown size={10} className="shrink-0 text-fg-dim" />
      </button>
      <div className="text-[9px] text-fg-dim mt-0.5 px-1">{label}</div>
      {open &&
        (() => {
          const rect = ref.current?.getBoundingClientRect();
          return createPortal(
            <div
              ref={portalRef}
              className="fixed glass-panel rounded-lg py-1 shadow-xl z-[100] max-h-[240px] overflow-y-auto"
              style={{
                top: rect ? rect.bottom + 4 : 0,
                left: rect?.left ?? 0,
                width: rect?.width ?? "auto",
              }}
            >
              {LANGUAGES.map((lang) => (
                <button
                  key={lang.code}
                  type="button"
                  onClick={() => {
                    onChange(lang.code);
                    setOpen(false);
                  }}
                  className={`w-full text-left px-2 py-1 text-ui-xs transition-colors ${
                    lang.code === value
                      ? "text-fg bg-control-hover"
                      : "text-fg-secondary hover:bg-control-hover hover:text-fg"
                  }`}
                >
                  {lang.flag} {lang.native}
                </button>
              ))}
            </div>,
            document.body,
          );
        })()}
    </div>
  );
}
