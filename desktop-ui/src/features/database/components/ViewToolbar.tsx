import { Button } from "@shared/ui/Button";
import { Input } from "@shared/ui/Input";

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
    <div className="flex items-center gap-3 py-2">
      <Input
        value={searchQuery}
        onChange={(e) => onSearchChange(e.target.value)}
        placeholder="Filter…"
        className="w-52"
      />
      {entityCount != null && (
        <span className="text-xs text-muted-foreground tabular-nums">{entityCount} items</span>
      )}
      <div className="flex-1" />
      <Button variant="primary" size="sm" onClick={onNewEntity}>
        + New
      </Button>
    </div>
  );
}
