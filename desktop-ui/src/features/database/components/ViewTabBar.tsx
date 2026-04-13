import type { ViewDefinition, ViewType } from "@shared/types";
import { Button } from "@shared/ui/Button";

const VIEW_TYPE_ICONS: Record<ViewType, string> = {
  table: "☰",
  board: "⊞",
  calendar: "📅",
  list: "≡",
  gallery: "⊡",
  timeline: "─",
};

interface ViewTabBarProps {
  views: ViewDefinition[];
  activeViewId: string | undefined;
  onViewSelect: (viewId: string) => void;
  onAddView?: () => void;
}

export function ViewTabBar({ views, activeViewId, onViewSelect, onAddView }: ViewTabBarProps) {
  return (
    <div className="flex items-center gap-0.5 pb-1">
      {views.map((view) => (
        <Button
          key={view.id}
          variant={activeViewId === view.id ? "secondary" : "ghost"}
          size="sm"
          onClick={() => onViewSelect(view.id)}
        >
          <span className="text-xs">{VIEW_TYPE_ICONS[view.viewType]}</span>
          {view.name}
        </Button>
      ))}
      {onAddView && (
        <Button variant="ghost" size="xs" onClick={onAddView}>
          +
        </Button>
      )}
    </div>
  );
}
