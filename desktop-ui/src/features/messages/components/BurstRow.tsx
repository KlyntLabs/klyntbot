import { memo } from "react";
import type { BurstGroup } from "../utils/groupBursts";
import { ToolRow } from "./MessageRows";
import { toolRowDescriptor } from "../utils/messageRenderUtils";

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
          className="tool-row__toggle"
          aria-label={`Toggle ${group.name} burst`}
          aria-expanded={expanded}
          onClick={() => onToggle(group.id)}
        >
          <span className="tool-row__icon" aria-hidden />
          <span className="tool-row__name">{group.name}:</span>
          <span className="tool-row__arg">
            {group.items.length} {group.name === "Read" ? "files" : "items"}
          </span>
          <span className="tool-row__meta">{preview}</span>
          <span className="tool-row__chevron" aria-hidden>
            ▸
          </span>
        </button>
      </div>
      {expanded && (
        <div className="tool-row__burst-children">
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
