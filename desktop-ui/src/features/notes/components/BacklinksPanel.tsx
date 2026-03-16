import { formatRelativeTime } from "@shared/lib/dates";
import { tagBgColor, tagColor } from "@shared/lib/tagColor";
import { ChevronDown, ChevronRight } from "lucide-react";
import { useState } from "react";
import { useBacklinks } from "../hooks/useBacklinks";

interface BacklinksPanelProps {
  noteId: string | null;
  onSelectNote: (id: string) => void;
}

export function BacklinksPanel({ noteId, onSelectNote }: BacklinksPanelProps) {
  const { data: backlinks } = useBacklinks(noteId);
  const [collapsed, setCollapsed] = useState(false);

  return (
    <div className="border-b border-border">
      <button
        type="button"
        onClick={() => setCollapsed(!collapsed)}
        className="w-full flex items-center gap-1.5 px-3 py-2 text-[10px] font-medium uppercase tracking-wider text-muted hover:text-secondary transition-colors"
      >
        {collapsed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
        <span>Backlinks ({backlinks.length})</span>
      </button>

      {!collapsed && (
        <div className="px-3 pb-2.5">
          {backlinks.length === 0 ? (
            <div className="text-[11px] text-dim py-1">No backlinks yet</div>
          ) : (
            <div className="flex flex-col gap-1">
              {backlinks.map((bl) => (
                <button
                  key={bl.note.id}
                  type="button"
                  onClick={() => onSelectNote(bl.note.id)}
                  className="w-full text-left rounded-md px-2 py-1.5 hover:bg-white/[0.04] transition-colors group"
                >
                  <div className="flex items-center gap-1.5">
                    <span className="text-[12px] text-secondary group-hover:text-primary truncate flex-1">
                      {bl.note.title}
                    </span>
                    <span className="text-[10px] text-dim shrink-0">
                      {formatRelativeTime(bl.note.updatedAt)}
                    </span>
                  </div>
                  {bl.context && (
                    <div className="text-[10px] text-dim mt-0.5 truncate">{bl.context}</div>
                  )}
                  {bl.note.tags.length > 0 && (
                    <div className="flex gap-1 mt-1 flex-wrap">
                      {bl.note.tags.slice(0, 3).map((tag) => (
                        <span
                          key={tag}
                          className="text-[9px] px-1 py-0.5 rounded"
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
                </button>
              ))}
            </div>
          )}

          {/* Phase 2 placeholder */}
          <div className="mt-3 pt-2 border-t border-white/[0.04]">
            <div className="text-[10px] font-medium uppercase tracking-wider text-dim">
              Unlinked mentions{" "}
              <span className="text-[9px] normal-case tracking-normal">(coming soon)</span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
