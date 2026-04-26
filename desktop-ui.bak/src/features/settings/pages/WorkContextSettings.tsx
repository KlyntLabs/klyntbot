import { SettingsCard } from "@shared/composites";
import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import { useToastContext } from "@shared/hooks/useToast";
import { SaveButton, Toggle } from "@shared/ui";
import { useState } from "react";
import { COGNITIVE_MODELS } from "../shared/cognitive-models";

// ── Types ────────────────────────────────────────────────────────────

interface WorkContextData {
  enabled?: boolean;
  inferenceIntervalMins?: number;
  maxActiveContexts?: number;
  semanticWeight?: number;
  temporalWeight?: number;
  resourceWeight?: number;
}

// Mirrors crates/config/src/schema/cognitive.rs — only the fields we expose in the UI.
interface CognitiveData {
  queryEnhancement?: {
    enabled?: boolean;
    prf?: {
      enabled?: boolean;
      initialFetchLimit?: number;
      minScoreThreshold?: number;
      maxExpansionTerms?: number;
    };
    multiQuery?: { enabled?: boolean; maxVariants?: number; model?: string };
    reranking?: { enabled?: boolean; llmRerankTopN?: number; llmRerankModel?: string };
    budgetOverrides?: {
      normal?: { maxLatencyMs?: number; maxLlmCalls?: number };
      deepThink?: { maxLatencyMs?: number; maxLlmCalls?: number };
      ultra?: { maxLatencyMs?: number; maxLlmCalls?: number };
    };
  };
}

interface AgentsData {
  defaults?: { provider?: string };
}

type DepthModeKey = "normal" | "deepThink" | "ultra";
const DEPTH_MODES: { key: DepthModeKey; label: string }[] = [
  { key: "normal", label: "Normal" },
  { key: "deepThink", label: "DeepThink" },
  { key: "ultra", label: "Ultra" },
];

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
  "w-full px-3 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim";

// ── Component ────────────────────────────────────────────────────────

export function WorkContextSettings() {
  const toast = useToastContext();
  const { data: config, refetch } = useQuery<WorkContextData>(
    "config_get_section",
    { section: "workContext" },
    {},
  );
  const { data: cognitive, refetch: refetchCognitive } = useQuery<CognitiveData>(
    "config_get_section",
    { section: "cognitive" },
    {},
  );
  const { data: agents } = useQuery<AgentsData>("config_get_section", { section: "agents" }, {});
  const providerName = agents?.defaults?.provider ?? "anthropic";
  const cognitiveModelOptions = COGNITIVE_MODELS[providerName] ?? [];

  const [edits, setEdits] = useState<Record<string, unknown>>({});
  const [qeEdits, setQeEdits] = useState<Record<string, unknown>>({});
  const [saving, setSaving] = useState(false);

  const val = <T,>(key: string, fallback: T): T => {
    if (key in edits) return edits[key] as T;
    return ((config as Record<string, unknown>)?.[key] as T) ?? fallback;
  };

  const setEdit = (key: string, value: unknown) => {
    setEdits((prev) => ({ ...prev, [key]: value }));
  };

  // Query-enhancement uses flat dotted keys ("prf.enabled") matching the
  // convention in PersonalizationSettings.tsx. Nested structure is rebuilt at save.
  const qeVal = <T,>(dottedKey: string, fallback: T): T => {
    if (dottedKey in qeEdits) return qeEdits[dottedKey] as T;
    const parts = dottedKey.split(".");
    let node: unknown = cognitive?.queryEnhancement;
    for (const p of parts) {
      if (node && typeof node === "object" && p in node) {
        node = (node as Record<string, unknown>)[p];
      } else {
        return fallback;
      }
    }
    return (node as T) ?? fallback;
  };

  const setQeEdit = (dottedKey: string, value: unknown) => {
    setQeEdits((prev) => ({ ...prev, [dottedKey]: value }));
  };

  const isDirty = Object.keys(edits).length > 0;
  const qeIsDirty = Object.keys(qeEdits).length > 0;

  const save = async () => {
    if (!isDirty && !qeIsDirty) return;
    setSaving(true);
    try {
      const calls: Promise<unknown>[] = [];
      if (isDirty) {
        calls.push(ipc("config_update_section", { section: "workContext", patch: edits }));
      }
      if (qeIsDirty) {
        // Rebuild nested shape from flat dotted keys
        const qePatch: Record<string, unknown> = {};
        for (const [key, value] of Object.entries(qeEdits)) {
          const parts = key.split(".");
          let node = qePatch;
          for (let i = 0; i < parts.length - 1; i++) {
            const p = parts[i];
            if (!(p in node) || typeof node[p] !== "object") node[p] = {};
            node = node[p] as Record<string, unknown>;
          }
          node[parts[parts.length - 1]] = value;
        }
        calls.push(
          ipc("config_update_section", {
            section: "cognitive",
            patch: { queryEnhancement: qePatch },
          }),
        );
      }
      await Promise.all(calls);
      setEdits({});
      setQeEdits({});
      if (isDirty) refetch();
      if (qeIsDirty) refetchCognitive();
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
                <span className="text-xs text-muted-foreground">Enable work contexts</span>
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
                        className={`text-xs font-medium ${active ? "text-brand" : "text-foreground"}`}
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

        <SettingsCard title="Query Enhancement">
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <div>
                <span className="text-xs text-muted-foreground">Enable query enhancement</span>
                <p className="text-[11px] text-dim">
                  Multi-stage retrieval pipeline that enriches queries and reranks results
                </p>
              </div>
              <Toggle checked={qeVal("enabled", true)} onChange={(v) => setQeEdit("enabled", v)} />
            </div>

            <div className="pt-2 border-t border-border">
              <div className="flex items-center justify-between">
                <div>
                  <span className="text-xs text-muted-foreground">Pseudo-relevance feedback</span>
                  <p className="text-[11px] text-dim">
                    Heuristic expansion using terms from initial retrieval results
                  </p>
                </div>
                <Toggle
                  checked={qeVal("prf.enabled", true)}
                  onChange={(v) => setQeEdit("prf.enabled", v)}
                />
              </div>

              <label className="block mt-3">
                <span className="block text-[11px] text-muted-foreground mb-0.5">
                  PRF initial fetch limit
                </span>
                <p className="text-[11px] text-dim mb-1">
                  How many facts to fetch for term extraction
                </p>
                <input
                  type="number"
                  min={2}
                  max={10}
                  value={qeVal("prf.initialFetchLimit", 3)}
                  onChange={(e) => {
                    const n = Number.parseInt(e.target.value, 10);
                    if (!Number.isNaN(n)) setQeEdit("prf.initialFetchLimit", n);
                  }}
                  className={`${INPUT_CLASS} w-24`}
                />
              </label>

              <label className="block mt-3">
                <span className="block text-[11px] text-muted-foreground mb-0.5">
                  PRF score threshold
                </span>
                <p className="text-[11px] text-dim mb-1">
                  Minimum score for a fact to contribute expansion terms
                </p>
                <input
                  type="number"
                  min={0.3}
                  max={0.9}
                  step={0.05}
                  value={qeVal("prf.minScoreThreshold", 0.6)}
                  onChange={(e) => {
                    const n = Number.parseFloat(e.target.value);
                    if (!Number.isNaN(n)) setQeEdit("prf.minScoreThreshold", n);
                  }}
                  className={`${INPUT_CLASS} w-24`}
                />
              </label>

              <label className="block mt-3">
                <span className="block text-[11px] text-muted-foreground mb-0.5">
                  PRF max expansion terms
                </span>
                <p className="text-[11px] text-dim mb-1">
                  How many terms to extract from initial results
                </p>
                <input
                  type="number"
                  min={2}
                  max={8}
                  value={qeVal("prf.maxExpansionTerms", 5)}
                  onChange={(e) => {
                    const n = Number.parseInt(e.target.value, 10);
                    if (!Number.isNaN(n)) setQeEdit("prf.maxExpansionTerms", n);
                  }}
                  className={`${INPUT_CLASS} w-24`}
                />
              </label>
            </div>

            <div className="pt-2 border-t border-border">
              <div className="flex items-center justify-between">
                <div>
                  <span className="text-xs text-muted-foreground">
                    Multi-query expansion (Deep+)
                  </span>
                  <p className="text-[11px] text-dim">
                    LLM generates query variants — only runs in DeepThink/Ultra modes
                  </p>
                </div>
                <Toggle
                  checked={qeVal("multiQuery.enabled", true)}
                  onChange={(v) => setQeEdit("multiQuery.enabled", v)}
                />
              </div>

              <label className="block mt-3">
                <span className="block text-[11px] text-muted-foreground mb-0.5">
                  Multi-query max variants
                </span>
                <p className="text-[11px] text-dim mb-1">
                  Number of alternative query formulations to generate
                </p>
                <input
                  type="number"
                  min={1}
                  max={5}
                  value={qeVal("multiQuery.maxVariants", 3)}
                  onChange={(e) => {
                    const n = Number.parseInt(e.target.value, 10);
                    if (!Number.isNaN(n)) setQeEdit("multiQuery.maxVariants", n);
                  }}
                  className={`${INPUT_CLASS} w-24`}
                />
              </label>

              <label className="block mt-3">
                <span className="block text-[11px] text-muted-foreground mb-0.5">
                  Multi-query model override
                </span>
                <p className="text-[11px] text-dim mb-1">
                  Leave unset to inherit the rewriter model
                </p>
                <select
                  value={qeVal("multiQuery.model", "") as string}
                  onChange={(e) => setQeEdit("multiQuery.model", e.target.value || null)}
                  className={`${INPUT_CLASS} w-full`}
                >
                  <option value="">(default)</option>
                  {cognitiveModelOptions.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                      {opt.recommended ? " (recommended)" : ""}
                    </option>
                  ))}
                </select>
              </label>
            </div>

            <div className="pt-2 border-t border-border">
              <div className="flex items-center justify-between">
                <div>
                  <span className="text-xs text-muted-foreground">Result reranking</span>
                  <p className="text-[11px] text-dim">
                    Heuristic rerank always runs; LLM rerank only in DeepThink/Ultra modes
                  </p>
                </div>
                <Toggle
                  checked={qeVal("reranking.enabled", true)}
                  onChange={(v) => setQeEdit("reranking.enabled", v)}
                />
              </div>

              <label className="block mt-3">
                <span className="block text-[11px] text-muted-foreground mb-0.5">
                  LLM rerank top-N
                </span>
                <p className="text-[11px] text-dim mb-1">
                  How many top results to send to the LLM for pairwise reranking
                </p>
                <input
                  type="number"
                  min={5}
                  max={20}
                  value={qeVal("reranking.llmRerankTopN", 10)}
                  onChange={(e) => {
                    const n = Number.parseInt(e.target.value, 10);
                    if (!Number.isNaN(n)) setQeEdit("reranking.llmRerankTopN", n);
                  }}
                  className={`${INPUT_CLASS} w-24`}
                />
              </label>

              <label className="block mt-3">
                <span className="block text-[11px] text-muted-foreground mb-0.5">
                  LLM rerank model override
                </span>
                <p className="text-[11px] text-dim mb-1">
                  Leave unset to inherit the rewriter model
                </p>
                <select
                  value={qeVal("reranking.llmRerankModel", "") as string}
                  onChange={(e) => setQeEdit("reranking.llmRerankModel", e.target.value || null)}
                  className={`${INPUT_CLASS} w-full`}
                >
                  <option value="">(default)</option>
                  {cognitiveModelOptions.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                      {opt.recommended ? " (recommended)" : ""}
                    </option>
                  ))}
                </select>
              </label>
            </div>

            <div className="pt-2 border-t border-border">
              <div>
                <span className="text-xs text-muted-foreground">Budget overrides</span>
                <p className="text-[11px] text-dim">
                  Per-depth-mode latency + LLM call ceilings. Leave blank for defaults.
                </p>
              </div>

              {DEPTH_MODES.map(({ key, label }) => (
                <div key={key} className="mt-3 space-y-2">
                  <span className="block text-[11px] text-muted-foreground">{label}</span>
                  <div className="flex gap-3">
                    <label className="block flex-1">
                      <span className="block text-[11px] text-dim mb-0.5">max latency (ms)</span>
                      <input
                        type="number"
                        min={50}
                        max={5000}
                        placeholder="default"
                        value={qeVal(`budgetOverrides.${key}.maxLatencyMs`, "") ?? ""}
                        onChange={(e) => {
                          const raw = e.target.value;
                          if (raw === "") {
                            setQeEdit(`budgetOverrides.${key}.maxLatencyMs`, null);
                            return;
                          }
                          const n = Number.parseInt(raw, 10);
                          if (!Number.isNaN(n)) {
                            setQeEdit(`budgetOverrides.${key}.maxLatencyMs`, n);
                          }
                        }}
                        className={`${INPUT_CLASS} w-full`}
                      />
                    </label>
                    <label className="block flex-1">
                      <span className="block text-[11px] text-dim mb-0.5">max LLM calls</span>
                      <input
                        type="number"
                        min={0}
                        max={10}
                        placeholder="default"
                        value={qeVal(`budgetOverrides.${key}.maxLlmCalls`, "") ?? ""}
                        onChange={(e) => {
                          const raw = e.target.value;
                          if (raw === "") {
                            setQeEdit(`budgetOverrides.${key}.maxLlmCalls`, null);
                            return;
                          }
                          const n = Number.parseInt(raw, 10);
                          if (!Number.isNaN(n)) {
                            setQeEdit(`budgetOverrides.${key}.maxLlmCalls`, n);
                          }
                        }}
                        className={`${INPUT_CLASS} w-full`}
                      />
                    </label>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </SettingsCard>

        {(isDirty || qeIsDirty) && <SaveButton onClick={save} saving={saving} />}
      </div>
    </div>
  );
}
