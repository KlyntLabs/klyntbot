import { memo } from "react";
import type { BurstGroup } from "../utils/groupBursts";
import { toolRowDescriptor } from "../utils/messageRenderUtils";
import { ToolRow } from "./MessageRows";

const PREVIEW_PATHS = 3;

type BurstRowProps = {
  group: BurstGroup;
  expandedItems: Set<string>;
  onToggle: (id: string) => void;
};

function basenameOf(path: string): string {
  const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

export const BurstRow = memo(function BurstRow({ group, expandedItems, onToggle }: BurstRowProps) {
  const expanded = expandedItems.has(group.id);
  const previewArgs = group.items
    .slice(0, PREVIEW_PATHS)
    .map((item) => basenameOf(toolRowDescriptor(item).arg))
    .filter(Boolean);
  const remaining = group.items.length - previewArgs.length;
  const preview = previewArgs.join(" · ") + (remaining > 0 ? ` · +${remaining} more` : "");

  return (
    <>
      <div
        className={`tool-row tool-row--${group.family} is-burst${expanded ? " is-expanded" : ""}`}
      >
        <button
          type="button"
          className="flex flex-1 items-center gap-[10px] py-2 pr-3 pl-3.5 bg-transparent border-0 cursor-pointer text-left [font:inherit] text-text-stronger min-w-0"
          aria-label={`Toggle ${group.name} burst`}
          aria-expanded={expanded}
          onClick={() => onToggle(group.id)}
        >
          <span className="w-3.5 h-3.5 shrink-0 inline-flex items-center justify-center" style={{ color: "var(--tool-row-bar)" }} aria-hidden />
          <span className="font-semibold text-ui-sm text-text-strong shrink-0">{group.name}:</span>
          <span className="font-code text-[11px] text-text-muted overflow-hidden text-ellipsis whitespace-nowrap min-w-0 flex-1">
            {group.items.length} {group.name === "Read" ? "files" : "items"}
          </span>
          <span className="ml-auto text-ui-xs text-text-faint inline-flex items-center gap-1.5 shrink-0">{preview}</span>
          <span className="shrink-0 text-text-subtle text-[10px] transition-transform duration-150" aria-hidden>
            ▸
          </span>
        </button>
      </div>
      {expanded && (
        <div
          className="flex flex-col gap-[2px] -mt-0.5 mb-1 ml-[26px] py-1 pl-3"
          style={{ borderLeft: "2px solid var(--tool-row-bar, var(--border-subtle))" }}
        >
          {group.items.map((item) => (
            <ToolRow
              key={item.id}
              item={item}
              isExpanded={expandedItems.has(item.id)}
              onToggle={onToggle}
            />
          ))}
        </div>
      )}
    </>
  );
});
