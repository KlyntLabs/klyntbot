import { ChevronDown, ChevronRight, Link2, MessageSquare, Zap } from "lucide-react";
import { useState } from "react";
import { useNoteSuggestions } from "../hooks/useNoteSuggestions";

interface AISuggestionsPanelProps {
  noteId: string | null;
}

const ACCENT = "rgba(167, 139, 250, 0.85)";
const ACCENT_BG = "rgba(139, 92, 246, 0.08)";

export function AISuggestionsPanel({ noteId }: AISuggestionsPanelProps) {
  const { suggestions } = useNoteSuggestions(noteId);
  const [collapsed, setCollapsed] = useState(false);

  // Suppress unused variable lint — will be used when backend is ready
  void suggestions;

  return (
    <div className="border-b border-border" style={{ borderLeftColor: ACCENT, borderLeftWidth: 2 }}>
      <button
        type="button"
        onClick={() => setCollapsed(!collapsed)}
        className="w-full flex items-center gap-1.5 px-3 py-2 text-[10px] font-medium uppercase tracking-wider text-muted hover:text-secondary transition-colors"
      >
        {collapsed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
        <span style={{ color: ACCENT }}>AI Suggestions</span>
      </button>

      {!collapsed && (
        <div className="px-3 pb-3" style={{ backgroundColor: ACCENT_BG }}>
          {/* Related Notes */}
          <div className="py-1.5">
            <div className="text-[10px] font-medium text-dim uppercase tracking-wider mb-1">
              Related Notes
            </div>
            <div className="text-[10px] text-dim italic">Semantic suggestions coming soon</div>
          </div>

          {/* Link Suggestions */}
          <div className="py-1.5 border-t border-white/[0.04]">
            <div className="text-[10px] font-medium text-dim uppercase tracking-wider mb-1">
              Link Suggestions
            </div>
            <div className="text-[10px] text-dim italic">Link analysis coming soon</div>
          </div>

          {/* Suggested Tags */}
          <div className="py-1.5 border-t border-white/[0.04]">
            <div className="text-[10px] font-medium text-dim uppercase tracking-wider mb-1">
              Suggested Tags
            </div>
            <div className="text-[10px] text-dim italic">Tag suggestions coming soon</div>
          </div>

          {/* Action buttons */}
          <div className="flex gap-1.5 mt-2 pt-2 border-t border-white/[0.04]">
            <button
              type="button"
              disabled
              className="flex items-center gap-1 text-[10px] px-2 py-1 rounded-md bg-white/[0.04] text-dim cursor-not-allowed"
            >
              <Zap size={10} />
              Synthesize
            </button>
            <button
              type="button"
              disabled
              className="flex items-center gap-1 text-[10px] px-2 py-1 rounded-md bg-white/[0.04] text-dim cursor-not-allowed"
            >
              <MessageSquare size={10} />
              Ask AI
            </button>
            <button
              type="button"
              disabled
              className="flex items-center gap-1 text-[10px] px-2 py-1 rounded-md bg-white/[0.04] text-dim cursor-not-allowed"
            >
              <Link2 size={10} />
              Create linked
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
