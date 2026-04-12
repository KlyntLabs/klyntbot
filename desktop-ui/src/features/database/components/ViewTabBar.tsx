import type { ViewDefinition, ViewType } from "@shared/types";

const VIEW_TYPE_ICONS: Record<ViewType, string> = {
  table: "\u2630",
  board: "\u229E",
  calendar: "\uD83D\uDCC5",
  list: "\u2261",
  gallery: "\u22A1",
  timeline: "\u2500",
};

interface ViewTabBarProps {
  views: ViewDefinition[];
  activeViewId: string | undefined;
  onViewSelect: (viewId: string) => void;
  onAddView?: () => void;
}

export function ViewTabBar({ views, activeViewId, onViewSelect, onAddView }: ViewTabBarProps) {
  return (
    <div className="flex items-center gap-1 border-b border-border px-4">
      {views.map((view) => (
        <button
          key={view.id}
          type="button"
          onClick={() => onViewSelect(view.id)}
          className={`flex items-center gap-1.5 border-b-2 px-3 py-2 text-sm transition-colors ${
            activeViewId === view.id
              ? "border-accent text-accent"
              : "border-transparent text-muted hover:text-foreground"
          }`}
        >
          <span>{VIEW_TYPE_ICONS[view.viewType]}</span>
          {view.name}
        </button>
      ))}
      {onAddView && (
        <button
          type="button"
          onClick={onAddView}
          className="px-2 py-2 text-sm text-muted hover:text-foreground"
        >
          +
        </button>
      )}
    </div>
  );
}
