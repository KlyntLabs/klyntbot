import { useEffect, useRef } from "react";
import { useLauncherStore } from "../stores/launcherStore";

export function LauncherInput() {
  const inputRef = useRef<HTMLInputElement>(null);
  const query = useLauncherStore((s) => s.query);
  const setQuery = useLauncherStore((s) => s.setQuery);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  return (
    <div className="flex items-center gap-3 px-4 py-3 border-b border-border">
      <svg
        className="w-5 h-5 text-muted shrink-0"
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
      <input
        ref={inputRef}
        type="text"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="Search apps, tasks, notes, or ask AI..."
        className="flex-1 bg-transparent text-sm text-foreground placeholder:text-muted outline-none"
        spellCheck={false}
        autoComplete="off"
      />
      {query && (
        <button
          type="button"
          onClick={() => setQuery("")}
          className="text-muted hover:text-foreground text-xs"
        >
          ESC
        </button>
      )}
    </div>
  );
}
