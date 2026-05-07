import { useMemo } from "react";
import type { MessageDto } from "../hooks/useThreadEvents";
import { AgentsMdPanel } from "./AgentsMdPanel";
import { PartRenderer } from "./parts";
import { SubagentTray } from "./SubagentTray";

type Props = {
  items: MessageDto[];
  threadId?: string;
  instructionSources?: Array<{ path: string; dir: string; contents: string }>;
};

function partKey(itemId: string, part: unknown, index: number): string {
  if (typeof part === "string") {
    return `${itemId}-part-${part.slice(0, 32)}`;
  }
  if (part && typeof part === "object" && "type" in part) {
    return `${itemId}-part-${(part as { type: string }).type}-${index}`;
  }
  return `${itemId}-part-${index}`;
}

/// Coding-thread item list — renders each MessageDto by dispatching its
/// `parts` array through `PartRenderer`. Pure presentational.
export function ThreadItemList({ items, threadId, instructionSources }: Props) {
  const itemsWithKeys = useMemo(
    () =>
      items.map((item) => ({
        ...item,
        partKeys: item.parts.map((part, index) => partKey(item.id, part, index)),
      })),
    [items],
  );

  return (
    <div className="thread-layout">
      <ol className="thread-item-list" aria-label="Coding thread items">
        {itemsWithKeys.map((item) => (
          <li
            key={item.id}
            className={`thread-item thread-item--${item.role}`}
            data-turn-id={item.turn_id ?? undefined}
          >
            <header className="thread-item__role">{item.role}</header>
            <div className="thread-item__parts">
              {item.parts.map((part, index) => (
                <PartRenderer key={item.partKeys[index]} part={part} />
              ))}
            </div>
          </li>
        ))}
      </ol>
      {threadId && (
        <aside className="thread-layout__side">
          <AgentsMdPanel threadId={threadId} initialSources={instructionSources ?? []} />
          <SubagentTray threadId={threadId} />
        </aside>
      )}
    </div>
  );
}
