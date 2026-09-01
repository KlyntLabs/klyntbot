import { formatRelativeTime } from "@shared/lib/dates";
import type { WorkContext } from "@shared/types";
import { Search } from "lucide-react";
import { useState } from "react";
import { contextColor } from "../lib/context-colors";

interface ContextSidebarProps {
  contexts: WorkContext[];
  selectedId?: string;
  onSelect: (ctx: WorkContext) => void;
  onSearchClick: () => void;
}

export function ContextSidebar({
  contexts,
  selectedId,
  onSelect,
  onSearchClick,
}: ContextSidebarProps) {
  const [collapsed, setCollapsed] = useState(false);

  return (
    <div className="flex flex-col">
      <button
        type="button"
        onClick={() => setCollapsed(!collapsed)}
        className="flex items-center justify-between px-1 py-1.5 text-[11px] font-medium text-dim uppercase tracking-wider hover:text-muted-foreground transition-colors"
      >
        <span>Work Contexts</span>
        <span className="text-2xs text-muted-foreground">{collapsed ? "▸" : "▾"}</span>
      </button>

      {!collapsed && (
        <div className="flex flex-col gap-0.5">
          {contexts.length === 0 && (
            <p className="text-[11px] text-muted-foreground px-1 py-2">No active contexts</p>
          )}
          {contexts.map((ctx) => {
            const isActive = selectedId === ctx.id;
            const color = contextColor(ctx.color, ctx.contextType);
            const ago = formatRelativeTime(ctx.lastActiveAt);
            const isRecent = ago === "now";

            return (
              <button
                key={ctx.id}
                type="button"
                onClick={() => onSelect(ctx)}
                className={`flex items-center gap-2 px-2 py-1.5 rounded-lg text-left transition-all ${
                  isActive
                    ? "bg-muted text-foreground"
                    : "text-muted-foreground hover:bg-card hover:text-foreground"
                }`}
              >
                <div
                  className={`size-2 rounded-full shrink-0 ${isRecent ? "animate-pulse" : ""}`}
                  style={{ backgroundColor: color }}
                />
                <div className="flex-1 min-w-0">
                  <p className="text-xs font-medium truncate">{ctx.title}</p>
                  <p className="text-2xs text-muted-foreground">
                    {isRecent ? "Active now" : `${ago} ago`}
                  </p>
                </div>
                <span className="text-2xs text-muted-foreground bg-accent rounded-full px-1.5 py-0.5 shrink-0">
                  {ctx.eventCount}
                </span>
              </button>
            );
          })}

          <button
            type="button"
            onClick={onSearchClick}
            className="flex items-center gap-1.5 px-2 py-1.5 rounded-lg text-[11px] text-muted-foreground hover:text-foreground hover:bg-card transition-colors mt-1"
          >
            <Search className="size-3" />
            Search contexts…
          </button>
        </div>
      )}
    </div>
  );
}
