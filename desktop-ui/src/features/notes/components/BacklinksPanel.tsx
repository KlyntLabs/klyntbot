import { formatRelativeTime } from "@shared/lib/dates";
import { tagBgColor, tagColor } from "@shared/lib/tagColor";
import { ChevronDown, ChevronRight } from "lucide-react";
import { useState } from "react";
import { useBacklinks } from "../hooks/useBacklinks";
import { useUnlinkedMentions } from "../hooks/useUnlinkedMentions";

interface BacklinksPanelProps {
  noteId: string | null;
  onSelectNote: (id: string) => void;
}

export function BacklinksPanel({ noteId, onSelectNote }: BacklinksPanelProps) {
  const { data: backlinks } = useBacklinks(noteId);
  const { data: unlinkedMentions } = useUnlinkedMentions(noteId);
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
                  className="w-full text-left rounded-md px-2 py-1.5 hover:bg-surface-low transition-colors group"
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

          {/* Unlinked Mentions */}
          <div className="mt-3 pt-2 border-t border-border-subtle">
            <div className="text-[10px] font-medium text-dim uppercase tracking-wider mb-1">
              Unlinked Mentions ({unlinkedMentions.length})
            </div>
            {unlinkedMentions.length > 0 ? (
              <div className="flex flex-col gap-0.5">
                {unlinkedMentions.map((note) => (
                  <button
                    key={note.id}
                    type="button"
                    onClick={() => onSelectNote(note.id)}
                    className="text-[11px] text-secondary hover:text-primary text-left px-1 py-0.5 rounded hover:bg-surface-low truncate transition-colors"
                  >
                    {note.title}
                  </button>
                ))}
              </div>
            ) : (
              <div className="text-[10px] text-dim italic">No unlinked mentions</div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
