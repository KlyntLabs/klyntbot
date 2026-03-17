import { useEffect, useRef } from "react";
import { useLauncherStore } from "../stores/launcherStore";
import type { LauncherItem } from "../types";

interface ResultsListProps {
  onExecute: (index: number) => void;
}

export function ResultsList({ onExecute }: ResultsListProps) {
  const results = useLauncherStore((s) => s.results);
  const selectedIndex = useLauncherStore((s) => s.selectedIndex);
  const isSearching = useLauncherStore((s) => s.isSearching);
  const listRef = useRef<HTMLDivElement>(null);

  // Scroll selected item into view
  useEffect(() => {
    const list = listRef.current;
    if (!list) return;
    const selected = list.children[selectedIndex] as HTMLElement | undefined;
    selected?.scrollIntoView({ block: "nearest" });
  }, [selectedIndex]);

  if (isSearching && results.length === 0) {
    return (
      <div className="p-4 flex items-center justify-center gap-2">
        <div className="flex gap-1">
          <div
            className="w-1.5 h-1.5 rounded-full bg-muted/60 animate-bounce"
            style={{ animationDelay: "0ms" }}
          />
          <div
            className="w-1.5 h-1.5 rounded-full bg-muted/60 animate-bounce"
            style={{ animationDelay: "150ms" }}
          />
          <div
            className="w-1.5 h-1.5 rounded-full bg-muted/60 animate-bounce"
            style={{ animationDelay: "300ms" }}
          />
        </div>
        <span className="text-muted-foreground text-sm">Searching...</span>
      </div>
    );
  }

  if (results.length === 0) {
    return <div className="p-4 text-center text-muted-foreground text-sm">No results</div>;
  }

  return (
    <div ref={listRef} className="max-h-[500px] overflow-y-auto py-1">
      {results.map((item, index) => (
        <ResultRow
          key={item.id}
          item={item}
          index={index}
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
  index,
  isSelected,
  onClick,
  onMouseEnter,
}: {
  item: LauncherItem;
  index: number;
  isSelected: boolean;
  onClick: () => void;
  onMouseEnter: () => void;
}) {
  return (
    <div
      role="option"
      tabIndex={-1}
      aria-selected={isSelected}
      className={`flex items-center gap-3 px-4 py-2 cursor-pointer transition-colors duration-100 animate-[result-in_0.15s_ease-out_both] ${
        isSelected ? "bg-muted" : "hover:bg-muted/50"
      }`}
      style={{ animationDelay: `${Math.min(index * 20, 200)}ms` }}
      onClick={onClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClick();
        }
      }}
      onMouseEnter={onMouseEnter}
    >
      <ItemIcon kind={item.kind.type} icon={item.icon} />
      <div className="flex-1 min-w-0">
        <div className="text-sm text-foreground truncate">{item.title}</div>
        {item.subtitle && (
          <div className="text-xs text-muted-foreground truncate">{item.subtitle}</div>
        )}
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
  file: "\uD83D\uDCC4",
  contentMatch: "\uD83D\uDD0D",
  contact: "\uD83D\uDC64",
  systemPref: "\u2699\uFE0F",
  runningApp: "\uD83D\uDFE2",
  bookmark: "\uD83D\uDD16",
  browserHistory: "\uD83C\uDF10",
  brewPackage: "\uD83C\uDF7A",
  sshHost: "\uD83D\uDD11",
  gitRepo: "\uD83D\uDCC2",
  urlNavigation: "\uD83C\uDF10",
};

function ItemIcon({ kind, icon }: { kind: string; icon?: string | null }) {
  if (icon?.startsWith("data:")) {
    return <img src={icon} alt="" className="w-6 h-6 shrink-0 rounded" />;
  }
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
  file: "File",
  contentMatch: "Match",
  contact: "Contact",
  systemPref: "Pref",
  runningApp: "Running",
  bookmark: "Bookmark",
  browserHistory: "History",
  brewPackage: "Brew",
  sshHost: "SSH",
  gitRepo: "Repo",
  urlNavigation: "URL",
};

function KindBadge({ type }: { type: string }) {
  return (
    <span className="text-[10px] text-muted-foreground px-1.5 py-0.5 rounded bg-accent shrink-0">
      {KIND_LABELS[type] || type}
    </span>
  );
}
