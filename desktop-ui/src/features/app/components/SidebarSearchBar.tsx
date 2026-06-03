import X from "lucide-react/dist/esm/icons/x";
import { cn } from "@/utils/cn";

type SidebarSearchBarProps = {
  isSearchOpen: boolean;
  searchQuery: string;
  onSearchQueryChange: (value: string) => void;
  onClearSearch: () => void;
};

export function SidebarSearchBar({
  isSearchOpen,
  searchQuery,
  onSearchQueryChange,
  onClearSearch,
}: SidebarSearchBarProps) {
  return (
    <div
      className={cn(
        "sidebar-search sticky top-0 z-[4] px-[4px]",
        isSearchOpen ? "pt-[6px] pb-[8px]" : "py-0 h-0 overflow-hidden",
      )}
    >
      {isSearchOpen && (
        <input
          className="sidebar-search-input w-full py-[10px] pr-[32px] pl-[14px] rounded-[14px] border border-border-quiet bg-[var(--cm-surface-panel-strong)] text-text-strong text-ui-sm outline-none placeholder:text-text-muted focus:border-border-accent focus:ring-2 focus:ring-border-accent-soft"
          value={searchQuery}
          onChange={(event) => onSearchQueryChange(event.target.value)}
          placeholder="Search conversations"
          aria-label="Search conversations"
          data-tauri-drag-region="false"
        />
      )}
      {isSearchOpen && searchQuery.length > 0 && (
        <button
          type="button"
          className="sidebar-search-clear absolute right-[14px] top-1/2 -translate-y-1/2 w-5 h-5 inline-flex items-center justify-center border-0 bg-transparent text-text-muted text-lg leading-none font-semibold cursor-pointer p-0 rounded-none shadow-none"
          onClick={onClearSearch}
          aria-label="Clear search"
          data-tauri-drag-region="false"
        >
          <X size={12} aria-hidden />
        </button>
      )}
    </div>
  );
}
