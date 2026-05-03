import { PartRenderer } from "./parts";
import type { MessageDto } from "../hooks/useThreadEvents";

type Props = {
  items: MessageDto[];
};

/// Coding-thread item list — renders each MessageDto by dispatching its
/// `parts` array through `PartRenderer`. Pure presentational.
export function ThreadItemList({ items }: Props) {
  return (
    <ol className="thread-item-list" aria-label="Coding thread items">
      {items.map((item) => (
        <li
          key={item.id}
          className={`thread-item thread-item--${item.role}`}
          data-turn-id={item.turn_id ?? undefined}
        >
          <header className="thread-item__role">{item.role}</header>
          <div className="thread-item__parts">
            {item.parts.map((part, idx) => (
              <PartRenderer key={`${item.id}-${idx}`} part={part} />
            ))}
          </div>
        </li>
      ))}
    </ol>
  );
}
