import { useRef, type ReactNode } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";

const DEFAULT_ESTIMATE_SIZE = 80;
const OVERSCAN = 5;

export type VirtualizedMessageListProps<T> = {
  items: T[];
  renderItem: (item: T, index: number) => ReactNode;
  getItemKey: (item: T, index: number) => string;
  scrollContainerRef?: React.RefObject<HTMLDivElement | null>;
  estimateSize?: number;
  /** Disable virtualization for small lists (default: ≤ 30 items). */
  disableThreshold?: number;
  /** Extra content rendered after the virtualized list. */
  trailingContent?: ReactNode;
};

/**
 * Virtualized message list using `@tanstack/react-virtual`.
 *
 * For short lists (≤ disableThreshold) items are rendered directly to avoid
 * virtualizer overhead. For long lists, only visible items are mounted in the
 * DOM, with `measureElement` tracking actual row heights.
 */
export function VirtualizedMessageList<T>({
  items,
  renderItem,
  getItemKey,
  scrollContainerRef,
  estimateSize = DEFAULT_ESTIMATE_SIZE,
  disableThreshold = 30,
  trailingContent,
}: VirtualizedMessageListProps<T>) {
  const fallbackRef = useRef<HTMLDivElement>(null);
  const scrollRef = scrollContainerRef ?? fallbackRef;

  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => estimateSize,
    overscan: OVERSCAN,
    getItemKey: (index) => getItemKey(items[index], index),
    measureElement:
      typeof window !== "undefined" && "ResizeObserver" in window
        ? (el) => el.getBoundingClientRect().height
        : undefined,
  });

  const virtualItems = virtualizer.getVirtualItems();

  if (items.length <= disableThreshold) {
    return (
      <>
        {items.map((item, i) => (
          <div key={getItemKey(item, i)}>{renderItem(item, i)}</div>
        ))}
        {trailingContent}
      </>
    );
  }

  return (
    <div
      style={{
        height: `${virtualizer.getTotalSize()}px`,
        width: "100%",
        position: "relative",
      }}
    >
      <div
        style={{
          position: "absolute",
          top: 0,
          left: 0,
          width: "100%",
          transform: `translateY(${virtualItems[0]?.start ?? 0}px)`,
        }}
      >
        {virtualItems.map((virtualItem) => {
          const item = items[virtualItem.index];
          if (!item) return null;
          return (
            <div
              key={virtualItem.key}
              data-index={virtualItem.index}
              ref={virtualizer.measureElement}
              style={{
                display: "flex",
                flexDirection: "column",
              }}
            >
              {renderItem(item, virtualItem.index)}
            </div>
          );
        })}
      </div>
      {trailingContent}
    </div>
  );
}
