import { AutoTunerPanel } from "@features/autotuner";
import { SettingsCard } from "@shared/composites";
import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import { useToastContext } from "@shared/hooks/useToast";
import { SaveButton, Toggle } from "@shared/ui";
import { useState } from "react";
import { COGNITIVE_MODELS } from "../shared/cognitive-models";

const PROVIDERS = [
  { value: "anthropic", label: "Anthropic" },
  { value: "openai", label: "OpenAI" },
  { value: "openrouter", label: "OpenRouter" },
  { value: "deepseek", label: "DeepSeek" },
  { value: "gemini", label: "Google Gemini" },
  { value: "groq", label: "Groq" },
  { value: "vllm", label: "vLLM" },
  { value: "zhipu", label: "Zhipu" },
  { value: "dashscope", label: "DashScope" },
  { value: "moonshot", label: "Moonshot" },
  { value: "minimax", label: "MiniMax" },
  { value: "aihubmix", label: "AIHubMix" },
] as const;

const MODEL_PRESETS: Record<string, { value: string; label: string }[]> = {
  anthropic: [
    { value: "anthropic/claude-opus-4-5", label: "Claude Opus 4.5" },
    { value: "anthropic/claude-sonnet-4-5", label: "Claude Sonnet 4.5" },
    { value: "anthropic/claude-haiku-3-5", label: "Claude Haiku 3.5" },
  ],
  openai: [
    { value: "openai/gpt-4o", label: "GPT-4o" },
    { value: "openai/gpt-4o-mini", label: "GPT-4o Mini" },
  ],
  deepseek: [
    { value: "deepseek/deepseek-chat", label: "DeepSeek Chat" },
    { value: "deepseek/deepseek-reasoner", label: "DeepSeek Reasoner" },
  ],
  gemini: [
    { value: "gemini/gemini-2.5-pro", label: "Gemini 2.5 Pro" },
    { value: "gemini/gemini-2.5-flash", label: "Gemini 2.5 Flash" },
  ],
};

const MAX_TOKEN_OPTIONS = [
  { value: 2048, label: "2,048" },
  { value: 4096, label: "4,096" },
  { value: 8192, label: "8,192" },
  { value: 16384, label: "16,384" },
  { value: 32768, label: "32,768" },
];

const ANALYSIS_INTERVAL_OPTIONS = [
  { value: 900, label: "15 minutes" },
  { value: 1800, label: "30 minutes" },
  { value: 3600, label: "1 hour" },
  { value: 7200, label: "2 hours" },
  { value: 14400, label: "4 hours" },
];

interface AgentsConfig {
  defaults?: {
    model?: string;
    provider?: string;
    temperature?: number;
    maxTokens?: number;
  };
}

interface CognitiveData {
  provider?: string;
  model?: string;
  temperature?: number;
  maxTokens?: number;
  intelligenceMode?: string;
  atomExtraction?: { enabled?: boolean };
}

interface LearningData {
  enabled?: boolean;
  analysisIntervalSecs?: number;
  minThreshold?: number;
  maxThreshold?: number;
  minOutcomesForAdaptation?: number;
}

interface ProviderManagerData {
  primary?: string;
  fallback?: string;
  classifierModel?: string;
}

export function AiSettings() {
  const toast = useToastContext();

  const { data: agents, refetch: refetchAgents } = useQuery<AgentsConfig>(
    "config_get_section",
    { section: "agents" },
    { defaults: {} },
  );

  const { data: cognitive, refetch: refetchCognitive } = useQuery<CognitiveData>(
    "config_get_section",
    { section: "cognitive" },
    {},
  );

  const { data: learning, refetch: refetchLearning } = useQuery<LearningData>(
    "config_get_section",
    { section: "learning" },
    { enabled: true },
  );

  const { data: providerManager, refetch: refetchPm } = useQuery<ProviderManagerData>(
    "config_get_section",
    { section: "providerManager" },
    {},
  );

  const defaults = agents.defaults ?? {};
  const [agentEdits, setAgentEdits] = useState<Record<string, unknown>>({});
  const [savingAgent, setSavingAgent] = useState(false);

  const agentVal = <T,>(key: string, fallback: T): T => {
    if (key in agentEdits) return agentEdits[key] as T;
    return ((defaults as Record<string, unknown>)[key] ?? fallback) as T;
  };

  const hasAgentChanges = Object.keys(agentEdits).length > 0;

  const activeProvider = agentVal("provider", "") as string;
  const modelOptions = MODEL_PRESETS[activeProvider] ?? [];

  const saveAgentDefaults = async () => {
    setSavingAgent(true);
    try {
      await ipc("config_update_section", {
        section: "agents",
        patch: { defaults: agentEdits },
      });
      refetchAgents();
      setAgentEdits({});
    } catch {
      toast.show("Failed to save agent defaults");
    } finally {
      setSavingAgent(false);
    }
  };

  const [pmEdits, setPmEdits] = useState<Record<string, unknown>>({});
  const [savingPm, setSavingPm] = useState(false);

  const pmVal = (key: string): string => {
    if (key in pmEdits) return (pmEdits[key] ?? "") as string;
    return ((providerManager as Record<string, unknown>)[key] ?? "") as string;
  };

  const hasPmChanges = Object.keys(pmEdits).length > 0;

  const savePm = async () => {
    setSavingPm(true);
    try {
      const patch: Record<string, unknown> = {};
      for (const [k, v] of Object.entries(pmEdits)) patch[k] = v || null;
      await ipc("config_update_section", { section: "providerManager", patch });
      setPmEdits({});
      refetchPm();
    } catch {
      toast.show("Failed to save provider routing");
    } finally {
      setSavingPm(false);
    }
  };

  const [cogEdits, setCogEdits] = useState<Record<string, unknown>>({});
  const [savingCog, setSavingCog] = useState(false);

  const cogProvider = (
    "provider" in cogEdits ? cogEdits.provider : (cognitive.provider ?? "")
  ) as string;
  const cogModel = ("model" in cogEdits ? cogEdits.model : (cognitive.model ?? "")) as string;
  const effectiveCogProvider = cogProvider || activeProvider;
  const cogModelOptions = COGNITIVE_MODELS[effectiveCogProvider] ?? [];

  const hasCogChanges = Object.keys(cogEdits).length > 0;

  const saveCognitive = async () => {
    setSavingCog(true);
    try {
      const patch: Record<string, unknown> = {};
      if ("provider" in cogEdits) patch.provider = cogEdits.provider || null;
      if ("model" in cogEdits) patch.model = cogEdits.model || null;
      if ("temperature" in cogEdits) patch.temperature = cogEdits.temperature;
      if ("maxTokens" in cogEdits) patch.maxTokens = cogEdits.maxTokens;
      if ("atomExtraction.enabled" in cogEdits) {
        patch.atomExtraction = { enabled: cogEdits["atomExtraction.enabled"] };
      }
      if ("intelligenceMode" in cogEdits) patch.intelligenceMode = cogEdits.intelligenceMode;
      await ipc("config_update_section", { section: "cognitive", patch });
      refetchCognitive();
      setCogEdits({});
    } catch {
      toast.show("Failed to save cognitive config");
    } finally {
      setSavingCog(false);
    }
  };

  const [learnEdits, setLearnEdits] = useState<Record<string, unknown>>({});
  const [savingLearn, setSavingLearn] = useState(false);

  const learnVal = <T,>(key: string, fallback: T): T => {
    if (key in learnEdits) return learnEdits[key] as T;
    return ((learning as Record<string, unknown>)[key] ?? fallback) as T;
  };

  const hasLearnChanges = Object.keys(learnEdits).length > 0;

  const saveLearn = async () => {
    setSavingLearn(true);
    try {
      await ipc("config_update_section", { section: "learning", patch: learnEdits });
      refetchLearning();
      setLearnEdits({});
    } catch {
      toast.show("Failed to save learning config");
    } finally {
      setSavingLearn(false);
    }
  };

  const selectClass =
    "w-full px-3 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors";

  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-foreground">AI</h2>
        <p className="text-[13px] text-muted-foreground mt-1">
          Model defaults, cognitive pipeline, learning, and optimization
        </p>
      </div>

      <div className="space-y-4">
        <SettingsCard title="Agent Defaults">
          <div className="space-y-3">
            <label className="block">
              <span className="block text-[11px] text-muted-foreground mb-1">Provider</span>
              <select
                value={agentVal("provider", "")}
                onChange={(e) => setAgentEdits((prev) => ({ ...prev, provider: e.target.value }))}
                className={selectClass}
              >
                <option value="" className="bg-popover">
                  Auto-detect
                </option>
                {PROVIDERS.map((p) => (
                  <option key={p.value} value={p.value} className="bg-popover">
                    {p.label}
                  </option>
                ))}
              </select>
            </label>

            <label className="block">
              <span className="block text-[11px] text-muted-foreground mb-1">Default model</span>
              {modelOptions.length > 0 ? (
                <select
                  value={agentVal("model", "")}
                  onChange={(e) => setAgentEdits((prev) => ({ ...prev, model: e.target.value }))}
                  className={selectClass}
                >
                  <option value="" className="bg-popover">
                    Default for provider
                  </option>
                  {modelOptions.map((m) => (
                    <option key={m.value} value={m.value} className="bg-popover">
                      {m.label}
                    </option>
                  ))}
                </select>
              ) : (
                <input
                  type="text"
                  value={agentVal("model", "")}
                  onChange={(e) => setAgentEdits((prev) => ({ ...prev, model: e.target.value }))}
                  placeholder="e.g. anthropic/claude-opus-4-5"
                  className={`${selectClass} placeholder:text-dim`}
                />
              )}
            </label>

            <div className="flex gap-3">
              <label className="flex-1">
                <span className="block text-[11px] text-muted-foreground mb-1">Temperature</span>
                <input
                  type="range"
                  min="0"
                  max="2"
                  step="0.1"
                  value={agentVal("temperature", 0.7)}
                  onChange={(e) =>
                    setAgentEdits((prev) => ({
                      ...prev,
                      temperature: Number.parseFloat(e.target.value),
                    }))
                  }
                  className="w-full accent-brand"
                />
                <span className="text-2xs text-dim">
                  {agentVal("temperature", 0.7).toFixed(1)}
                </span>
              </label>
              <label className="flex-1">
                <span className="block text-[11px] text-muted-foreground mb-1">Max tokens</span>
                <select
                  value={agentVal("maxTokens", 8192)}
                  onChange={(e) =>
                    setAgentEdits((prev) => ({
                      ...prev,
                      maxTokens: Number.parseInt(e.target.value, 10),
                    }))
                  }
                  className={selectClass}
                >
                  {MAX_TOKEN_OPTIONS.map((o) => (
                    <option key={o.value} value={o.value} className="bg-popover">
                      {o.label}
                    </option>
                  ))}
                </select>
              </label>
            </div>

            {hasAgentChanges && (
              <div className="flex justify-end">
                <SaveButton onClick={saveAgentDefaults} saving={savingAgent} />
              </div>
            )}
          </div>
        </SettingsCard>

        <SettingsCard title="Provider Routing">
          <div className="space-y-3">
            <p className="text-[11px] text-dim -mt-1">Automatic failover and routing</p>
            {(["primary", "fallback"] as const).map((field) => (
              <label key={field} className="block">
                <span className="block text-[11px] text-muted-foreground mb-1 capitalize">
                  {field} provider
                </span>
                <select
                  value={pmVal(field)}
                  onChange={(e) => setPmEdits((prev) => ({ ...prev, [field]: e.target.value }))}
                  className={selectClass}
                >
                  <option value="" className="bg-popover">
                    {field === "primary" ? "Auto (use agent default)" : "None"}
                  </option>
                  {PROVIDERS.map((p) => (
                    <option key={p.value} value={p.value} className="bg-popover">
                      {p.label}
                    </option>
                  ))}
                </select>
              </label>
            ))}
            {hasPmChanges && (
              <div className="flex justify-end">
                <SaveButton onClick={savePm} saving={savingPm} />
              </div>
            )}
          </div>
        </SettingsCard>

        <SettingsCard title="Cognitive Pipeline">
          <div className="space-y-3">
            <p className="text-[11px] text-dim -mt-1">
              Background AI for memory extraction, consolidation, and reflection
            </p>

            <label className="block">
              <span className="block text-[11px] text-muted-foreground mb-1">
                Provider override
              </span>
              <select
                value={cogProvider}
                onChange={(e) =>
                  setCogEdits((prev) => ({ ...prev, provider: e.target.value, model: "" }))
                }
                className={selectClass}
              >
                <option value="" className="bg-popover">
                  Same as main ({PROVIDERS.find((p) => p.value === activeProvider)?.label || "auto"}
                  )
                </option>
                {PROVIDERS.map((p) => (
                  <option key={p.value} value={p.value} className="bg-popover">
                    {p.label}
                  </option>
                ))}
              </select>
            </label>

            <div>
              <span className="block text-[11px] text-muted-foreground mb-1">Model</span>
              {cogModelOptions.length > 0 ? (
                <select
                  value={cogModel}
                  onChange={(e) => setCogEdits((prev) => ({ ...prev, model: e.target.value }))}
                  className={selectClass}
                >
                  <option value="" className="bg-popover">
                    Same as main agent model
                  </option>
                  {cogModelOptions.map((m) => (
                    <option key={m.value} value={m.value} className="bg-popover">
                      {m.label}
                      {m.recommended ? " ★" : ""}
                    </option>
                  ))}
                </select>
              ) : (
                <input
                  type="text"
                  value={cogModel}
                  onChange={(e) => setCogEdits((prev) => ({ ...prev, model: e.target.value }))}
                  placeholder="Leave blank to use main agent model"
                  className={`${selectClass} placeholder:text-dim`}
                />
              )}
            </div>

            <div className="flex items-center justify-between pt-1 border-t border-border">
              <div>
                <span className="text-xs text-muted-foreground">Deep Intelligence Mode</span>
                <p className="text-[11px] text-dim">Full LLM processing instead of heuristics</p>
              </div>
              <Toggle
                checked={
                  "intelligenceMode" in cogEdits
                    ? cogEdits.intelligenceMode === "deep"
                    : cognitive.intelligenceMode === "deep"
                }
                onChange={(v) =>
                  setCogEdits((prev) => ({ ...prev, intelligenceMode: v ? "deep" : "standard" }))
                }
              />
            </div>

            <div className="flex items-center justify-between pt-1 border-t border-border">
              <div>
                <span className="text-xs text-muted-foreground">Auto-extract knowledge atoms</span>
                <p className="text-[11px] text-dim">Extract concepts and facts from notes</p>
              </div>
              <Toggle
                checked={
                  "atomExtraction.enabled" in cogEdits
                    ? (cogEdits["atomExtraction.enabled"] as boolean)
                    : (cognitive.atomExtraction?.enabled ?? true)
                }
                onChange={(v) => setCogEdits((prev) => ({ ...prev, "atomExtraction.enabled": v }))}
              />
            </div>

            {hasCogChanges && (
              <div className="flex justify-end">
                <SaveButton onClick={saveCognitive} saving={savingCog} />
              </div>
            )}
          </div>
        </SettingsCard>

        <SettingsCard title="Learning & Adaptation">
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <div>
                <span className="text-xs text-muted-foreground">Enable learning</span>
                <p className="text-[11px] text-dim">Adaptive confidence thresholds</p>
              </div>
              <Toggle
                checked={learnVal("enabled", true)}
                onChange={(v) => setLearnEdits((prev) => ({ ...prev, enabled: v }))}
              />
            </div>

            <label className="block">
              <span className="block text-[11px] text-muted-foreground mb-1">
                Analysis interval
              </span>
              <select
                value={learnVal("analysisIntervalSecs", 3600)}
                onChange={(e) =>
                  setLearnEdits((prev) => ({
                    ...prev,
                    analysisIntervalSecs: Number.parseInt(e.target.value, 10),
                  }))
                }
                className={selectClass}
              >
                {ANALYSIS_INTERVAL_OPTIONS.map((o) => (
                  <option key={o.value} value={o.value} className="bg-popover">
                    {o.label}
                  </option>
                ))}
              </select>
            </label>

            <div className="flex gap-3">
              <label className="flex-1">
                <span className="block text-[11px] text-muted-foreground mb-1">Min threshold</span>
                <input
                  type="number"
                  value={learnVal("minThreshold", 0.4)}
                  onChange={(e) =>
                    setLearnEdits((prev) => ({
                      ...prev,
                      minThreshold: Number.parseFloat(e.target.value) || 0.4,
                    }))
                  }
                  step="0.05"
                  min="0"
                  max="1"
                  className={selectClass}
                />
              </label>
              <label className="flex-1">
                <span className="block text-[11px] text-muted-foreground mb-1">Max threshold</span>
                <input
                  type="number"
                  value={learnVal("maxThreshold", 0.9)}
                  onChange={(e) =>
                    setLearnEdits((prev) => ({
                      ...prev,
                      maxThreshold: Number.parseFloat(e.target.value) || 0.9,
                    }))
                  }
                  step="0.05"
                  min="0"
                  max="1"
                  className={selectClass}
                />
              </label>
            </div>

            {hasLearnChanges && (
              <div className="flex justify-end">
                <SaveButton onClick={saveLearn} saving={savingLearn} />
              </div>
            )}
          </div>
        </SettingsCard>

        <SettingsCard title="AutoTuner">
          <p className="text-[11px] text-dim mb-3">
            Continuous self-optimization via A/B experiments
          </p>
          <AutoTunerPanel />
        </SettingsCard>

        <InferenceSettingsCard />
      </div>
    </div>
  );
}

function InferenceSettingsCard() {
  const toast = useToastContext();
  const { data: config, refetch } = useQuery<Record<string, unknown>>(
    "config_get_section",
    { section: "inference" },
    {},
  );

  const [edits, setEdits] = useState<Record<string, unknown>>({});
  const [saving, setSaving] = useState(false);

  const val = (key: string, fallback: number): number => {
    if (key in edits) return edits[key] as number;
    return (config[key] as number) ?? fallback;
  };

  const hasChanges = Object.keys(edits).length > 0;

  const save = async () => {
    setSaving(true);
    try {
      await ipc("config_update_section", { section: "inference", patch: edits });
      refetch();
      setEdits({});
    } catch {
      toast.show("Failed to save inference config");
    } finally {
      setSaving(false);
    }
  };

  const selectClass =
    "w-full px-3 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors";

  return (
    <SettingsCard title="Inference Engine">
      <div className="space-y-3">
        <p className="text-[11px] text-dim -mt-1">Work context detection and assignment</p>

        <label className="block">
          <span className="block text-[11px] text-muted-foreground mb-1">Assignment threshold</span>
          <div className="flex items-center gap-2">
            <input
              type="range"
              min="0"
              max="1"
              step="0.05"
              value={val("assignmentThreshold", 0.6)}
              onChange={(e) =>
                setEdits((prev) => ({
                  ...prev,
                  assignmentThreshold: Number.parseFloat(e.target.value),
                }))
              }
              className="flex-1 accent-brand"
            />
            <span className="text-2xs text-dim w-8 text-right">
              {val("assignmentThreshold", 0.6)}
            </span>
          </div>
        </label>

        <label className="block">
          <span className="block text-[11px] text-muted-foreground mb-1">Merge threshold</span>
          <div className="flex items-center gap-2">
            <input
              type="range"
              min="0"
              max="1"
              step="0.05"
              value={val("mergeThreshold", 0.8)}
              onChange={(e) =>
                setEdits((prev) => ({
                  ...prev,
                  mergeThreshold: Number.parseFloat(e.target.value),
                }))
              }
              className="flex-1 accent-brand"
            />
            <span className="text-2xs text-dim w-8 text-right">{val("mergeThreshold", 0.8)}</span>
          </div>
        </label>

        <label className="block">
          <span className="block text-[11px] text-muted-foreground mb-1">Max active contexts</span>
          <input
            type="number"
            value={val("maxActiveContexts", 20)}
            onChange={(e) =>
              setEdits((prev) => ({
                ...prev,
                maxActiveContexts: Number.parseInt(e.target.value, 10) || 20,
              }))
            }
            min="5"
            max="100"
            className={selectClass}
          />
        </label>

        {hasChanges && (
          <div className="flex justify-end">
            <SaveButton onClick={save} saving={saving} />
          </div>
        )}
      </div>
    </SettingsCard>
  );
}
