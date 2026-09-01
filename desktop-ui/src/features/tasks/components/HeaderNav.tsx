import { Button } from "@shared/ui/Button";
import { Search, X } from "lucide-react";
import { useCallback } from "react";
import { useShallow } from "zustand/react/shallow";
import { useSearchStore } from "../store/search-store";
import { useTabStore } from "../store/tab-store";

export default function HeaderNav() {
  const { isSearchOpen, searchQuery, toggleSearch, closeSearch, setSearchQuery } = useSearchStore();
  const navStack = useTabStore(
    useShallow((s) => {
      const active = s.tabs.find((t) => t.id === s.activeTabId);
      return active?.navStack ?? [];
    }),
  );
  const navigateToStackIndex = useTabStore((s) => s.navigateToStackIndex);

  const inputRef = useCallback((node: HTMLInputElement | null) => {
    node?.focus();
  }, []);

  return (
    <div className="flex items-center justify-between px-4 py-2 border-b border-border">
      {/* Left — Breadcrumb */}
      <div className="flex items-center gap-1 min-w-0">
        {navStack.map((entry, index) => {
          const isLast = index === navStack.length - 1;
          return (
            <div
              key={`${entry.type}-${entry.targetId}`}
              className="flex items-center gap-1 min-w-0"
            >
              {index > 0 && <span className="text-xs text-muted-foreground flex-shrink-0">›</span>}
              {isLast ? (
                <span className="text-sm font-medium text-foreground truncate">{entry.label}</span>
              ) : (
                <button
                  type="button"
                  onClick={() => navigateToStackIndex(index)}
                  className="text-sm text-muted-foreground hover:text-foreground transition-colors truncate"
                >
                  {entry.label}
                </button>
              )}
            </div>
          );
        })}
      </div>

      {/* Right — Search */}
      <div className="flex items-center gap-2">
        {isSearchOpen ? (
          <div className="flex items-center gap-2">
            <div className="relative">
              <Search className="absolute left-2 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
              <input
                ref={inputRef}
                type="text"
                placeholder="Search issues..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="h-7 w-[200px] rounded-md border border-border bg-background pl-8 pr-2 text-sm text-foreground placeholder:text-muted-foreground outline-none focus:ring-1 focus:ring-brand"
              />
            </div>
            <Button size="xs" variant="ghost" onClick={closeSearch}>
              <X className="size-4" />
            </Button>
          </div>
        ) : (
          <Button size="xs" variant="ghost" onClick={toggleSearch}>
            <Search className="size-4" />
          </Button>
        )}
      </div>
    </div>
  );
}
