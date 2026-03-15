import { useEffect, useRef } from "react";
import { useLauncherStore } from "../stores/launcherStore";
import type { LauncherItem } from "../types";

interface ResultsListProps {
  onExecute: (index: number) => void;
}

export function ResultsList({ onExecute }: ResultsListProps) {
  const results = useLauncherStore((s) => s.results);
  const selectedIndex = useLauncherStore((s) => s.selectedIndex);
  const listRef = useRef<HTMLDivElement>(null);

  // Scroll selected item into view
  useEffect(() => {
    const list = listRef.current;
    if (!list) return;
    const selected = list.children[selectedIndex] as HTMLElement | undefined;
    selected?.scrollIntoView({ block: "nearest" });
  }, [selectedIndex]);

  if (results.length === 0) {
    return <div className="p-4 text-center text-muted text-sm">No results</div>;
  }

  return (
    <div ref={listRef} className="max-h-[500px] overflow-y-auto py-1">
      {results.map((item, index) => (
        <ResultRow
          key={item.id}
          item={item}
          isSelected={index === selectedIndex}
          onClick={() => onExecute(index)}
          onMouseEnter={() => useLauncherStore.getState().setSelectedIndex(index)}
        />
      ))}
    </div>
  );
}

function ResultRow({
  item,
  isSelected,
  onClick,
  onMouseEnter,
}: {
  item: LauncherItem;
  isSelected: boolean;
  onClick: () => void;
  onMouseEnter: () => void;
}) {
  return (
    <div
      role="option"
      tabIndex={-1}
      aria-selected={isSelected}
      className={`flex items-center gap-3 px-4 py-2 cursor-pointer transition-colors ${
        isSelected ? "bg-surface-raised" : "hover:bg-surface-raised/50"
      }`}
      onClick={onClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClick();
        }
      }}
      onMouseEnter={onMouseEnter}
    >
      <ItemIcon kind={item.kind.type} />
      <div className="flex-1 min-w-0">
        <div className="text-sm text-foreground truncate">{item.title}</div>
        {item.subtitle && <div className="text-xs text-muted truncate">{item.subtitle}</div>}
      </div>
      <KindBadge type={item.kind.type} />
    </div>
  );
}

const ICON_MAP: Record<string, string> = {
  application: "\uD83E\uDEDF",
  task: "\u2713",
  note: "\uD83D\uDCDD",
  clipboardEntry: "\uD83D\uDCCB",
  systemCommand: "\u2699\uFE0F",
  script: "\u25B6",
  calculator: "\uD83D\uDD22",
  calendar: "\uD83D\uDCC5",
  aiChat: "\u2728",
};

function ItemIcon({ kind }: { kind: string }) {
  return (
    <span className="w-6 h-6 flex items-center justify-center text-sm shrink-0">
      {ICON_MAP[kind] || "\u2022"}
    </span>
  );
}

const KIND_LABELS: Record<string, string> = {
  application: "App",
  task: "Task",
  note: "Note",
  clipboardEntry: "Clip",
  systemCommand: "Cmd",
  script: "Script",
  calculator: "Calc",
  calendar: "Event",
  aiChat: "AI",
};

function KindBadge({ type }: { type: string }) {
  return (
    <span className="text-[10px] text-muted px-1.5 py-0.5 rounded bg-surface-base shrink-0">
      {KIND_LABELS[type] || type}
    </span>
  );
}
