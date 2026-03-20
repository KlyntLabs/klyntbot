import { SettingsCard } from "@shared/composites";
import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import { useToastContext } from "@shared/hooks/useToast";
import { SaveButton, Toggle } from "@shared/ui";
import { useState } from "react";

// ── Types ────────────────────────────────────────────────────────────

interface WorkContextData {
  enabled?: boolean;
  inferenceIntervalMins?: number;
  maxActiveContexts?: number;
  semanticWeight?: number;
  temporalWeight?: number;
  resourceWeight?: number;
}

type Preset = "light" | "balanced" | "deep";

const PRESETS: Record<Preset, { semantic: number; temporal: number; resource: number }> = {
  light: { semantic: 0.0, temporal: 0.5, resource: 0.5 },
  balanced: { semantic: 0.3, temporal: 0.35, resource: 0.35 },
  deep: { semantic: 0.7, temporal: 0.15, resource: 0.15 },
};

const PRESET_OPTIONS: { value: Preset; label: string; description: string }[] = [
  { value: "light", label: "Light", description: "No embeddings — fastest, lowest CPU" },
  { value: "balanced", label: "Balanced", description: "Heuristics first, semantic as tiebreaker" },
  { value: "deep", label: "Deep", description: "Heavy semantic matching — best accuracy" },
];

const INPUT_CLASS =
  "w-full px-3 py-1.5 text-[12px] text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim";

// ── Component ────────────────────────────────────────────────────────

export function WorkContextSettings() {
  const toast = useToastContext();
  const { data: config, refetch } = useQuery<WorkContextData>(
    "config_get_section",
    { section: "workContext" },
    {},
  );

  const [edits, setEdits] = useState<Record<string, unknown>>({});
  const [saving, setSaving] = useState(false);

  // ── Helpers ──────────────────────────────────────────────────────

  const val = <T,>(key: string, fallback: T): T => {
    if (key in edits) return edits[key] as T;
    return ((config as Record<string, unknown>)?.[key] as T) ?? fallback;
  };

  const setEdit = (key: string, value: unknown) => {
    setEdits((prev) => ({ ...prev, [key]: value }));
  };

  const isDirty = Object.keys(edits).length > 0;

  const save = async () => {
    if (!isDirty) return;
    setSaving(true);
    try {
      await ipc("config_update_section", { section: "workContext", patch: edits });
      setEdits({});
      refetch();
    } catch {
      toast.show("Failed to save settings");
    } finally {
      setSaving(false);
    }
  };

  // ── Preset detection ──────────────────────────────────────────

  const currentSemantic = val("semanticWeight", 0.3);
  const activePreset: Preset | null = Object.entries(PRESETS).find(
    ([_, p]) => Math.abs(p.semantic - currentSemantic) < 0.01,
  )?.[0] as Preset | null;

  const applyPreset = (preset: Preset) => {
    const p = PRESETS[preset];
    setEdit("semanticWeight", p.semantic);
    setEdit("temporalWeight", p.temporal);
    setEdit("resourceWeight", p.resource);
  };

  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-foreground">Work Contexts</h2>
        <p className="text-[13px] text-muted-foreground mt-1">
          Automatic activity grouping and context inference
        </p>
      </div>

      <div className="space-y-4">
        {/* ── General ──────────────────────────────────────────── */}
        <SettingsCard title="General">
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <div>
                <span className="text-[12px] text-muted-foreground">Enable work contexts</span>
                <p className="text-[11px] text-dim">
                  Automatically track and group your activity into contexts
                </p>
              </div>
              <Toggle checked={val("enabled", true)} onChange={(v) => setEdit("enabled", v)} />
            </div>

            <label className="block">
              <span className="block text-[11px] text-muted-foreground mb-0.5">
                Inference interval (minutes)
              </span>
              <p className="text-[11px] text-dim mb-1">How often to process unassigned events</p>
              <input
                type="number"
                min={1}
                max={60}
                value={val("inferenceIntervalMins", 5)}
                onChange={(e) =>
                  setEdit("inferenceIntervalMins", Number.parseInt(e.target.value, 10) || 5)
                }
                className={`${INPUT_CLASS} w-24`}
              />
            </label>

            <label className="block">
              <span className="block text-[11px] text-muted-foreground mb-0.5">
                Maximum active contexts
              </span>
              <p className="text-[11px] text-dim mb-1">
                Oldest contexts are auto-archived when this limit is reached
              </p>
              <input
                type="number"
                min={5}
                max={200}
                value={val("maxActiveContexts", 50)}
                onChange={(e) =>
                  setEdit("maxActiveContexts", Number.parseInt(e.target.value, 10) || 50)
                }
                className={`${INPUT_CLASS} w-24`}
              />
            </label>
          </div>
        </SettingsCard>

        {/* ── Inference Mode ──────────────────────────────────── */}
        <SettingsCard title="Inference mode">
          <div className="space-y-3">
            <div>
              <span className="block text-[11px] text-muted-foreground mb-1">
                How events are matched to contexts
              </span>
              <p className="text-[11px] text-dim mb-2">
                Light skips embeddings entirely for lowest CPU. Deep uses semantic similarity for
                best accuracy.
              </p>
              <div className="flex flex-col gap-2">
                {PRESET_OPTIONS.map((opt) => {
                  const active = activePreset === opt.value;
                  return (
                    <button
                      type="button"
                      key={opt.value}
                      onClick={() => applyPreset(opt.value)}
                      className={`px-3 py-2 text-left rounded-lg border transition-colors ${
                        active
                          ? "bg-brand/10 border-brand/30"
                          : "bg-accent border-border hover:border-border-hover"
                      }`}
                    >
                      <span
                        className={`text-[12px] font-medium ${active ? "text-brand" : "text-foreground"}`}
                      >
                        {opt.label}
                      </span>
                      <p className="text-[11px] text-dim mt-0.5">{opt.description}</p>
                    </button>
                  );
                })}
              </div>
            </div>
          </div>
        </SettingsCard>

        {isDirty && <SaveButton onClick={save} saving={saving} />}
      </div>
    </div>
  );
}
