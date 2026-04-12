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
    <div className="flex items-center gap-3 border-b border-border px-4 py-2">
      <input
        type="text"
        value={searchQuery}
        onChange={(e) => onSearchChange(e.target.value)}
        placeholder="Search\u2026"
        className="w-48 rounded border border-border bg-surface-base px-2 py-1 text-sm outline-none focus:border-accent"
      />
      {entityCount != null && <span className="text-xs text-muted">{entityCount} items</span>}
      <div className="flex-1" />
      <button
        type="button"
        onClick={onNewEntity}
        className="rounded bg-accent px-3 py-1 text-sm text-white hover:bg-accent/90"
      >
        + New
      </button>
    </div>
  );
}
