import { useUpdateView } from "@features/database/hooks/useViews";
import { getChartConfig, withChartConfig } from "@features/database/lib/view-defaults";
import type {
  ChartConfig,
  DatabaseSchema,
  FilterGroup,
  SortRule,
  ViewConfig,
  ViewDefinition,
} from "@shared/types";
import { Checkbox } from "@shared/ui/Checkbox";
import { Plus, X } from "lucide-react";
import { useEffect, useState } from "react";
import { FilterBuilder } from "./filter/FilterBuilder";
import { VIEW_TYPE_LABELS, ViewTypeIcon } from "./ViewTypeIcon";

interface ViewConfigPanelProps {
  schema: DatabaseSchema;
  view: ViewDefinition;
  onClose: () => void;
}

export function ViewConfigPanel({ schema, view, onClose }: ViewConfigPanelProps) {
  const [name, setName] = useState(view.name);
  const updateView = useUpdateView(schema.id);

  // Only reset the local name buffer when switching views; never clobber in-flight edits
  // just because the server echoed back a refreshed `view`.
  useEffect(() => {
    setName(view.name);
  }, [view.id]);

  const config = view.config;

  const updateConfig = (patch: Partial<ViewConfig>) => {
    void updateView.mutate(view.id, { config: { ...config, ...patch } });
  };

  const defaultVisible = schema.fields.filter((f) => !f.hidden).map((f) => f.slug);
  const visibleSet = new Set(config.visibleFields ?? defaultVisible);

  const toggleVisible = (slug: string) => {
    const next = new Set(visibleSet);
    if (next.has(slug)) next.delete(slug);
    else next.add(slug);
    updateConfig({ visibleFields: Array.from(next) });
  };

  const groupableTypes = ["select", "multi_select", "checkbox"];
  const groupableFields = schema.fields.filter(
    (f) => !f.hidden && groupableTypes.includes(f.fieldType),
  );
  const canGroup = ["board", "table", "list", "gallery"].includes(view.viewType);
  const dateFields = schema.fields.filter((f) => !f.hidden && f.fieldType === "date");

  return (
    <div className="glass-card ml-2 mr-2 mb-2 flex w-80 shrink-0 flex-col overflow-hidden">
      <div className="flex shrink-0 items-center justify-between border-b border-border/60 px-4 py-2">
        <div className="flex items-center gap-1.5">
          <ViewTypeIcon type={view.viewType} />
          <h3 className="text-[13px] font-semibold text-foreground">
            {VIEW_TYPE_LABELS[view.viewType]} view
          </h3>
        </div>
        <button
          type="button"
          onClick={onClose}
          className="rounded p-1 text-foreground/60 hover:bg-accent hover:text-foreground"
          aria-label="Close view settings"
        >
          <X className="h-3.5 w-3.5" />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto p-3 space-y-4">
        <Section label="Name">
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            onBlur={() => {
              const trimmed = name.trim();
              if (trimmed && trimmed !== view.name) {
                void updateView.mutate(view.id, { name: trimmed });
              }
            }}
            className="w-full rounded-md border border-border bg-background px-2 py-1 text-[13px] outline-none focus:border-foreground/30"
          />
        </Section>

        {canGroup && groupableFields.length > 0 && (
          <Section label="Group by">
            <FieldSelect
              schema={schema}
              value={config.groupBy}
              onChange={(v) => updateConfig({ groupBy: v || undefined })}
              allowedTypes={groupableTypes}
            />
          </Section>
        )}

        {view.viewType === "chart" && (
          <Section label="Chart">
            <ChartConfigEditor
              schema={schema}
              config={getChartConfig(view.config)}
              onChange={(chart) => updateConfig(withChartConfig(view.config, chart))}
            />
          </Section>
        )}

        {(view.viewType === "calendar" || view.viewType === "timeline") &&
          dateFields.length > 0 && (
            <Section label={view.viewType === "timeline" ? "Start date" : "Date field"}>
              <FieldSelect
                schema={schema}
                value={config.calendarField}
                onChange={(v) => updateConfig({ calendarField: v })}
                allowedTypes={["date"]}
              />
            </Section>
          )}

        <Section label="Sort">
          <SortEditor
            schema={schema}
            sorts={config.sorts ?? []}
            onChange={(sorts) => updateConfig({ sorts })}
          />
        </Section>

        <Section label="Filter">
          <FilterBuilder
            schema={schema}
            value={config.filter}
            onChange={(filter: FilterGroup | undefined) => updateConfig({ filter })}
          />
        </Section>

        <Section label="Visible properties">
          <div className="space-y-0.5">
            {schema.fields
              .filter((f) => !f.hidden)
              .map((field) => (
                <label
                  key={field.slug}
                  className="flex cursor-pointer items-center gap-2 rounded px-1.5 py-1 text-[12px] text-foreground/85 hover:bg-accent"
                >
                  <Checkbox
                    checked={visibleSet.has(field.slug)}
                    onCheckedChange={() => toggleVisible(field.slug)}
                  />
                  <span className="truncate">{field.name}</span>
                </label>
              ))}
          </div>
        </Section>
      </div>
    </div>
  );
}

function Section({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <div className="mb-1.5 text-[11px] font-medium uppercase tracking-wide text-foreground/55">
        {label}
      </div>
      {children}
    </div>
  );
}

interface FieldSelectProps {
  schema: DatabaseSchema;
  value: string | undefined;
  onChange: (slug: string) => void;
  allowedTypes?: string[];
}

function FieldSelect({ schema, value, onChange, allowedTypes }: FieldSelectProps) {
  const fields = schema.fields.filter(
    (f) => !f.hidden && (!allowedTypes || allowedTypes.includes(f.fieldType)),
  );
  return (
    <select
      value={value ?? ""}
      onChange={(e) => onChange(e.target.value)}
      className="w-full rounded-md border border-border bg-background px-2 py-1 text-[13px] outline-none"
    >
      <option value="">—</option>
      {fields.map((f) => (
        <option key={f.slug} value={f.slug}>
          {f.name}
        </option>
      ))}
    </select>
  );
}

interface SortEditorProps {
  schema: DatabaseSchema;
  sorts: SortRule[];
  onChange: (sorts: SortRule[]) => void;
}

function SortEditor({ schema, sorts, onChange }: SortEditorProps) {
  const fields = schema.fields.filter((f) => !f.hidden);
  return (
    <div className="space-y-1">
      {sorts.map((sort, i) => (
        <div key={i} className="flex items-center gap-1">
          <select
            value={sort.field}
            onChange={(e) =>
              onChange(sorts.map((s, j) => (j === i ? { ...s, field: e.target.value } : s)))
            }
            className="flex-1 rounded border border-border bg-background px-1.5 py-0.5 text-[12px] outline-none"
          >
            {fields.map((f) => (
              <option key={f.slug} value={f.slug}>
                {f.name}
              </option>
            ))}
          </select>
          <select
            value={sort.direction}
            onChange={(e) =>
              onChange(
                sorts.map((s, j) =>
                  j === i ? { ...s, direction: e.target.value as "asc" | "desc" } : s,
                ),
              )
            }
            className="rounded border border-border bg-background px-1.5 py-0.5 text-[12px] outline-none"
          >
            <option value="asc">↑ Asc</option>
            <option value="desc">↓ Desc</option>
          </select>
          <button
            type="button"
            onClick={() => onChange(sorts.filter((_, j) => j !== i))}
            className="rounded p-0.5 text-foreground/45 hover:bg-red-500/10 hover:text-red-500"
            aria-label="Remove sort"
          >
            <X className="h-3 w-3" />
          </button>
        </div>
      ))}
      <button
        type="button"
        onClick={() => onChange([...sorts, { field: fields[0]?.slug ?? "", direction: "asc" }])}
        className="flex items-center gap-1 rounded px-1.5 py-0.5 text-[12px] text-foreground/60 hover:bg-accent hover:text-foreground"
      >
        <Plus className="h-3 w-3" /> Add sort
      </button>
    </div>
  );
}

interface ChartConfigEditorProps {
  schema: DatabaseSchema;
  config: ChartConfig | undefined;
  onChange: (next: ChartConfig) => void;
}

const SELECT_CLASS =
  "w-full rounded-md border border-border bg-background px-2 py-1 text-[13px] outline-none";

function ChartConfigEditor({ schema, config, onChange }: ChartConfigEditorProps) {
  const fields = schema.fields.filter((f) => !f.hidden);
  const xAxis = config?.xAxis ?? fields[0]?.slug ?? "";
  const chartType = config?.chartType ?? "bar";
  const aggregation = config?.aggregation ?? "count";
  const patch = (p: Partial<ChartConfig>) => onChange({ chartType, xAxis, aggregation, ...p });
  return (
    <div className="space-y-2">
      <select
        value={chartType}
        onChange={(e) => patch({ chartType: e.target.value as ChartConfig["chartType"] })}
        className={SELECT_CLASS}
      >
        <option value="bar">Bar</option>
        <option value="line">Line</option>
        <option value="pie">Pie</option>
      </select>
      <select
        value={xAxis}
        onChange={(e) => patch({ xAxis: e.target.value })}
        className={SELECT_CLASS}
      >
        {fields.map((f) => (
          <option key={f.slug} value={f.slug}>
            {f.name}
          </option>
        ))}
      </select>
      <select
        value={aggregation}
        onChange={(e) => patch({ aggregation: e.target.value as ChartConfig["aggregation"] })}
        className={SELECT_CLASS}
      >
        <option value="count">Count</option>
        <option value="sum" disabled>
          Sum (coming soon)
        </option>
        <option value="avg" disabled>
          Average (coming soon)
        </option>
      </select>
    </div>
  );
}
