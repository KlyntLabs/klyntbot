import { useMutation } from "@shared/hooks/useMutation";
import { invalidateQueries, useQuery } from "@shared/hooks/useQuery";
import { SaveButton } from "@shared/ui/SaveButton";
import { Activity, BarChart3, Merge, Target, Zap } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

interface InferenceStats {
  activeContextCount: number;
  archivedContextCount: number;
  eventsLastHour: number;
  eventsLast24h: number;
  assignmentRate: number;
  avgConfidence: number;
  mergesLast24h: number;
  lastRunAt: string | null;
}

interface InferenceConfig {
  assignmentThreshold: number;
  mergeThreshold: number;
  semanticWeight: number;
  temporalWeight: number;
  resourceWeight: number;
  inferenceIntervalMins: number;
  maxDormancyDays: number;
  maxActiveContexts: number;
}

function StatCard({
  label,
  value,
  icon: Icon,
  accent,
}: {
  label: string;
  value: string;
  icon: typeof Activity;
  accent?: string;
}) {
  return (
    <div className="glass-card rounded-xl p-4 flex items-center gap-3">
      <div
        className="w-9 h-9 rounded-lg flex items-center justify-center"
        style={{ backgroundColor: `${accent ?? "#6B7280"}20` }}
      >
        <Icon className="w-4.5 h-4.5" strokeWidth={1.5} style={{ color: accent ?? "#6B7280" }} />
      </div>
      <div>
        <p className="text-lg font-semibold text-primary leading-tight">{value}</p>
        <p className="text-[11px] text-muted">{label}</p>
      </div>
    </div>
  );
}

function Slider({
  label,
  value,
  onChange,
  min = 0,
  max = 1,
  step = 0.05,
  format,
}: {
  label: string;
  value: number;
  onChange: (v: number) => void;
  min?: number;
  max?: number;
  step?: number;
  format?: (v: number) => string;
}) {
  const display = format ? format(value) : value.toFixed(2);
  return (
    <div className="flex items-center gap-3">
      <span className="text-[12px] text-secondary w-40 shrink-0">{label}</span>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="flex-1 accent-accent"
      />
      <span className="text-[12px] text-muted w-12 text-right tabular-nums">{display}</span>
    </div>
  );
}

export function InferenceTab() {
  const { data: stats, refetch } = useQuery<InferenceStats>(
    "get_inference_stats",
    undefined,
    undefined,
    15_000,
  );

  // Load current config from settings
  const { data: config } = useQuery<InferenceConfig>("config_get_section", {
    section: "workContext",
  });

  const [draft, setDraft] = useState<InferenceConfig | null>(null);
  const saveMutation = useMutation("update_inference_config", "config");

  // Initialize draft from config
  useEffect(() => {
    if (config && !draft) {
      setDraft(config);
    }
  }, [config, draft]);

  const handleSave = useCallback(async () => {
    if (!draft) return;
    await saveMutation.mutate(draft);
    invalidateQueries("config_get_section");
  }, [draft, saveMutation]);

  const update = (key: keyof InferenceConfig, value: number) => {
    if (!draft) return;
    setDraft({ ...draft, [key]: value });
  };

  const pct = (v: number) => `${(v * 100).toFixed(0)}%`;
  const num = (v: number) => String(v);

  return (
    <div className="overflow-y-auto p-4 space-y-6">
      {/* Stats Grid */}
      <section>
        <h3 className="text-[11px] font-medium text-dim uppercase tracking-wider mb-3">
          Real-time Stats
        </h3>
        <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
          <StatCard
            label="Active Contexts"
            value={String(stats?.activeContextCount ?? 0)}
            icon={Target}
            accent="#8B5CF6"
          />
          <StatCard
            label="Archived"
            value={String(stats?.archivedContextCount ?? 0)}
            icon={Activity}
            accent="#6B7280"
          />
          <StatCard
            label="Events (1h)"
            value={String(stats?.eventsLastHour ?? 0)}
            icon={Zap}
            accent="#F59E0B"
          />
          <StatCard
            label="Events (24h)"
            value={String(stats?.eventsLast24h ?? 0)}
            icon={BarChart3}
            accent="#3B82F6"
          />
          <StatCard
            label="Assignment Rate"
            value={pct(stats?.assignmentRate ?? 0)}
            icon={Target}
            accent="#10B981"
          />
          <StatCard
            label="Avg Confidence"
            value={(stats?.avgConfidence ?? 0).toFixed(2)}
            icon={Activity}
            accent="#EC4899"
          />
          <StatCard
            label="Merges (24h)"
            value={String(stats?.mergesLast24h ?? 0)}
            icon={Merge}
            accent="#06B6D4"
          />
          <StatCard
            label="Last Run"
            value={stats?.lastRunAt ? new Date(stats.lastRunAt).toLocaleTimeString() : "—"}
            icon={Zap}
            accent="#6B7280"
          />
        </div>
        <button
          type="button"
          onClick={() => refetch()}
          className="mt-2 text-[11px] text-muted hover:text-secondary transition-colors"
        >
          Refresh
        </button>
      </section>

      {/* Config Section */}
      {draft && (
        <section>
          <h3 className="text-[11px] font-medium text-dim uppercase tracking-wider mb-3">
            Inference Configuration
          </h3>
          <div className="glass-card rounded-xl p-5 space-y-4">
            <Slider
              label="Assignment Threshold"
              value={draft.assignmentThreshold}
              onChange={(v) => update("assignmentThreshold", v)}
            />
            <Slider
              label="Merge Threshold"
              value={draft.mergeThreshold}
              onChange={(v) => update("mergeThreshold", v)}
            />

            <div className="border-t border-border my-3" />
            <p className="text-[11px] text-muted">Scoring weights (should sum to 1.0)</p>

            <Slider
              label="Semantic Weight"
              value={draft.semanticWeight}
              onChange={(v) => update("semanticWeight", v)}
            />
            <Slider
              label="Temporal Weight"
              value={draft.temporalWeight}
              onChange={(v) => update("temporalWeight", v)}
            />
            <Slider
              label="Resource Weight"
              value={draft.resourceWeight}
              onChange={(v) => update("resourceWeight", v)}
            />

            <div className="border-t border-border my-3" />

            <Slider
              label="Inference Interval (min)"
              value={draft.inferenceIntervalMins}
              onChange={(v) => update("inferenceIntervalMins", v)}
              min={1}
              max={30}
              step={1}
              format={num}
            />
            <Slider
              label="Max Dormancy (days)"
              value={draft.maxDormancyDays}
              onChange={(v) => update("maxDormancyDays", v)}
              min={1}
              max={30}
              step={1}
              format={num}
            />
            <Slider
              label="Max Active Contexts"
              value={draft.maxActiveContexts}
              onChange={(v) => update("maxActiveContexts", v)}
              min={5}
              max={100}
              step={5}
              format={num}
            />

            <div className="flex justify-end pt-2">
              <SaveButton onClick={handleSave} saving={saveMutation.loading} />
            </div>
          </div>
        </section>
      )}
    </div>
  );
}
