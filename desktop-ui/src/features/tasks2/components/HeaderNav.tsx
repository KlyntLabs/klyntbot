import { Search, X } from "lucide-react";
import { useCallback } from "react";
import { useSearchStore } from "../store/search-store";
import { Button } from "./ui/button";

export default function HeaderNav() {
  const { isSearchOpen, searchQuery, toggleSearch, closeSearch, setSearchQuery } = useSearchStore();

  const inputRef = useCallback((node: HTMLInputElement | null) => {
    node?.focus();
  }, []);

  return (
    <div className="flex items-center justify-between px-4 py-2 border-b border-[hsl(var(--border))]">
      {/* Left */}
      <div className="flex items-center gap-2">
        <h1 className="text-sm font-medium text-[hsl(var(--foreground))]">My Issues</h1>
      </div>

      {/* Right */}
      <div className="flex items-center gap-2">
        {isSearchOpen ? (
          <div className="flex items-center gap-2">
            <div className="relative">
              <Search className="absolute left-2 top-1/2 -translate-y-1/2 h-4 w-4 text-[hsl(var(--muted-foreground))]" />
              <input
                ref={inputRef}
                type="text"
                placeholder="Search issues..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="h-7 w-[200px] rounded-md border border-[hsl(var(--border))] bg-[hsl(var(--background))] pl-8 pr-2 text-sm text-[hsl(var(--foreground))] placeholder:text-[hsl(var(--muted-foreground))] outline-none focus:ring-1 focus:ring-[hsl(var(--ring))]"
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
