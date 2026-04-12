import type { FilterRule, WidgetDefinition } from "@shared/types";
import { CountWidget } from "./widgets/CountWidget";
import { ListWidget } from "./widgets/ListWidget";
import { MetricWidget } from "./widgets/MetricWidget";

interface DashboardBuilderProps {
  widgets: WidgetDefinition[];
}

export function DashboardBuilder({ widgets }: DashboardBuilderProps) {
  if (widgets.length === 0) {
    return (
      <div className="flex items-center justify-center h-64 text-muted">
        No widgets yet. Add one to get started.
      </div>
    );
  }

  return (
    <div className="grid grid-cols-4 gap-4 p-4 auto-rows-[160px]">
      {widgets.map((widget) => (
        <div
          key={widget.id}
          className="rounded-lg border border-border bg-surface-base shadow-sm overflow-hidden"
          style={{
            gridColumn: `${widget.position.col + 1} / span ${widget.position.width}`,
            gridRow: `${widget.position.row + 1} / span ${widget.position.height}`,
          }}
        >
          <WidgetRenderer widget={widget} />
        </div>
      ))}
    </div>
  );
}

function WidgetRenderer({ widget }: { widget: WidgetDefinition }) {
  const config = widget.config;
  switch (widget.widgetType) {
    case "count":
      return (
        <CountWidget
          databaseId={widget.databaseId}
          title={(config.title as string) ?? "Count"}
          filters={config.filters as FilterRule[] | undefined}
        />
      );
    case "list":
      return (
        <ListWidget
          databaseId={widget.databaseId}
          title={(config.title as string) ?? "Items"}
          limit={config.limit as number}
        />
      );
    case "metric":
      return (
        <MetricWidget
          databaseId={widget.databaseId}
          title={(config.title as string) ?? "Metric"}
          valueField={(config.valueField as string) ?? ""}
          aggregation={(config.aggregation as "sum" | "avg" | "min" | "max" | "count") ?? "sum"}
        />
      );
    default:
      return (
        <div className="flex items-center justify-center h-full text-sm text-muted">
          {widget.widgetType} widget (coming soon)
        </div>
      );
  }
}
