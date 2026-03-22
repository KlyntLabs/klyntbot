import { isTauri } from "@shared/lib/utils";
import { ThinkingDots } from "@shared/ui/ThinkingDots";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useRef } from "react";
import { useLauncherStore } from "../stores/launcherStore";

export function LauncherInput() {
  const inputRef = useRef<HTMLInputElement>(null);
  const query = useLauncherStore((s) => s.query);
  const setQuery = useLauncherStore((s) => s.setQuery);
  const isSearching = useLauncherStore((s) => s.isSearching);

  // Focus on mount
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Re-focus when the window becomes visible (global shortcut)
  useEffect(() => {
    if (!isTauri) return;
    let unlisten: (() => void) | undefined;
    getCurrentWindow()
      .onFocusChanged(({ payload: focused }) => {
        if (focused) {
          // Small delay lets the window finish animating before we grab focus
          setTimeout(() => inputRef.current?.focus(), 50);
        }
      })
      .then((fn) => {
        unlisten = fn;
      });
    return () => unlisten?.();
  }, []);

  return (
    <div className="flex items-center gap-3 px-4 py-3 border-b border-border">
      {isSearching ? (
        <div className="w-5 h-5 shrink-0 flex items-center justify-center">
          <ThinkingDots size="sm" />
        </div>
      ) : (
        <svg
          className="w-5 h-5 text-muted-foreground shrink-0"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={2}
          aria-hidden="true"
          role="img"
        >
          <title>Search</title>
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
          />
        </svg>
      )}
      <input
        ref={inputRef}
        type="text"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="Search apps, tasks, notes, or ask AI..."
        className="flex-1 bg-transparent text-sm text-foreground placeholder:text-muted-foreground outline-none"
        spellCheck={false}
        autoComplete="off"
      />
      {query && (
        <button
          type="button"
          onClick={() => setQuery("")}
          className="text-muted-foreground hover:text-foreground text-xs"
        >
          ESC
        </button>
      )}
    </div>
  );
}
