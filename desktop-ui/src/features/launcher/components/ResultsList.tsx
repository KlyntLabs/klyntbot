import { isTauri } from "@shared/lib/utils";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useRef } from "react";
import { useDndActive } from "../hooks/useDndActive";
import { formatRemaining } from "../lib/formatRemaining";
import { useLauncherStore } from "../stores/launcherStore";
import type { LauncherItem } from "../types";

interface ResultsListProps {
  onExecute: (index: number) => void;
}

export function ResultsList({ onExecute }: ResultsListProps) {
  const results = useLauncherStore((s) => s.results);
  const selectedIndex = useLauncherStore((s) => s.selectedIndex);
  const dndActive = useDndActive();
  const isSearching = useLauncherStore((s) => s.isSearching);
  const listRef = useRef<HTMLDivElement>(null);
  // Prevent hover selection when results render under a stationary cursor.
  // Only allow mouse-based selection after the mouse physically moves (>3px).
  // Typing-induced hand vibration fires mousemove events with sub-pixel
  // changes — tracking coordinates avoids those false positives.
  const mouseMovedRef = useRef(false);
  const lastMousePosRef = useRef<{ x: number; y: number } | null>(null);

  // Reset mouse-moved flag when results change — use first item ID as a
  // stable identity signal (length alone can't distinguish different result sets).
  const resultsKey = results.length > 0 ? results[0].id : "";
  // biome-ignore lint/correctness/useExhaustiveDependencies: intentionally reacting to result identity change
  useEffect(() => {
    mouseMovedRef.current = false;
    lastMousePosRef.current = null;
  }, [resultsKey]);

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
            className="w-1.5 h-1.5 rounded-full bg-control-hover/60 animate-bounce"
            style={{ animationDelay: "0ms" }}
          />
          <div
            className="w-1.5 h-1.5 rounded-full bg-control-hover/60 animate-bounce"
            style={{ animationDelay: "150ms" }}
          />
          <div
            className="w-1.5 h-1.5 rounded-full bg-control-hover/60 animate-bounce"
            style={{ animationDelay: "300ms" }}
          />
        </div>
        <span className="text-fg-secondary text-sm">Searching...</span>
      </div>
    );
  }

  if (results.length === 0) {
    return <div className="p-4 text-center text-fg-secondary text-sm">No results</div>;
  }

  return (
    <div
      ref={listRef}
      role="listbox"
      className="max-h-[500px] overflow-y-auto py-1"
      onMouseMove={(e) => {
        const last = lastMousePosRef.current;
        if (last && (Math.abs(e.clientX - last.x) > 3 || Math.abs(e.clientY - last.y) > 3)) {
          mouseMovedRef.current = true;
        }
        lastMousePosRef.current = { x: e.clientX, y: e.clientY };
      }}
    >
      {(() => {
        const uniqueGroups = new Set(results.map((item) => groupLabel(item.kind.type)));
        const showHeaders = uniqueGroups.size >= 2;
        let lastGroup = "";
        return results.map((item, index) => {
          const group = groupLabel(item.kind.type);
          const showHeader = showHeaders && group !== "" && group !== lastGroup;
          lastGroup = group;
          return (
            <div key={item.id}>
              {showHeader && (
                <div className="px-3 pt-2 pb-1 text-ui-xs font-medium text-fg-secondary uppercase tracking-wider">
                  {group}
                </div>
              )}
              <ResultRow
                item={item}
                index={index}
                isSelected={index === selectedIndex}
                dndSession={dndActive.data}
                onClick={() => onExecute(index)}
                onMouseEnter={() => {
                  if (mouseMovedRef.current) {
                    useLauncherStore.getState().setSelectedIndex(index);
                  }
                }}
              />
            </div>
          );
        });
      })()}
    </div>
  );
}

function groupLabel(kind: string): string {
  if (kind === "application" || kind === "runningApp") return "Apps";
  if (kind === "file" || kind === "contentMatch" || kind === "gitRepo") return "Files";
  if (kind === "task") return "Tasks";
  if (kind === "note") return "Notes";
  if (kind === "bookmark" || kind === "browserHistory" || kind === "urlNavigation") return "Web";
  return "";
}

function ResultRow({
  item,
  index,
  isSelected,
  dndSession,
  onClick,
  onMouseEnter,
}: {
  item: LauncherItem;
  index: number;
  isSelected: boolean;
  dndSession?: import("../types").FocusSession | null;
  onClick: () => void;
  onMouseEnter: () => void;
}) {
  const isDndItem = item.kind.type === "systemCommand" && item.kind.action === "toggleDoNotDisturb";
  const displayTitle =
    isDndItem && dndSession ? `DND on — ${formatRemaining(dndSession.endsAt)} left` : item.title;

  return (
    <div
      role="option"
      tabIndex={-1}
      aria-selected={isSelected}
      className={`flex items-center gap-3 px-4 py-2 cursor-pointer transition-colors duration-100 animate-[result-in_0.15s_ease-out_both] ${
        isSelected ? "bg-control-hover" : ""
      }`}
      style={{ animationDelay: `${Math.min(index * 10, 100)}ms` }}
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
        <div className="text-sm text-fg truncate">{displayTitle}</div>
        {item.subtitle && (
          <div className="text-ui-sm text-fg-secondary truncate">{item.subtitle}</div>
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
  // File path to cached PNG (sips-extracted app icon)
  if (icon?.startsWith("/")) {
    const src = isTauri ? convertFileSrc(icon) : icon;
    return <img src={src} alt="" className="size-6 shrink-0 rounded" loading="lazy" />;
  }
  // Legacy base64 data URI
  if (icon?.startsWith("data:")) {
    return <img src={icon} alt="" className="size-6 shrink-0 rounded" loading="lazy" />;
  }
  // Emoji fallback from ICON_MAP
  return (
    <span className="size-6 flex items-center justify-center text-sm shrink-0">
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
    <span className="text-ui-xs text-fg-secondary px-1.5 py-0.5 rounded bg-control-hover shrink-0">
      {KIND_LABELS[type] || type}
    </span>
  );
}
