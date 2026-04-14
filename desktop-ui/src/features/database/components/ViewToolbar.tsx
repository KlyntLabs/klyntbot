import { Button } from "@shared/ui/Button";

interface ViewToolbarProps {
  searchQuery: string;
  onSearchChange: (query: string) => void;
  onNewEntity: () => void;
  entityCount?: number;
}

export function ViewToolbar({
  searchQuery,
  onSearchChange,
  onNewEntity,
  entityCount,
}: ViewToolbarProps) {
  return (
    <div className="flex items-center gap-3">
      {/* Search — lightweight inline filter, Notion-style */}
      <div className="relative">
        <svg
          className="pointer-events-none absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-foreground/60"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
          strokeWidth={1.5}
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="m21 21-5.197-5.197m0 0A7.5 7.5 0 1 0 5.196 5.196a7.5 7.5 0 0 0 10.607 10.607Z"
          />
        </svg>
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => onSearchChange(e.target.value)}
          placeholder="Filter…"
          className="h-7 w-56 rounded-md border border-transparent bg-transparent py-1 pl-7 pr-2 text-[13px] text-foreground placeholder:text-foreground/45 outline-none transition-colors focus:border-border focus:bg-accent"
        />
      </div>

      {entityCount != null && (
        <span className="text-[12px] text-foreground/55 tabular-nums">{entityCount} items</span>
      )}

      <Button variant="primary" size="sm" onClick={onNewEntity}>
        <svg
          className="h-3.5 w-3.5"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
          strokeWidth={2}
        >
          <path strokeLinecap="round" strokeLinejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
        </svg>
        New
      </Button>
    </div>
  );
}
