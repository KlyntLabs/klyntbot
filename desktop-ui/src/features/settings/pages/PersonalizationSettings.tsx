import { SettingsCard } from "@shared/composites";
import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import { useToastContext } from "@shared/hooks/useToast";
import { SaveButton, SecretInput, Toggle } from "@shared/ui";
import { useState } from "react";
import { ThemeSwitcher } from "../components/ThemeSwitcher";

// ── Provider list ────────────────────────────────────────────────────

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

// Recommended cognitive models per provider — fast & cheap models ideal for
// extraction, consolidation, reflection, and coaching reasoning tasks.
const COGNITIVE_MODELS: Record<string, { value: string; label: string; recommended?: boolean }[]> =
  {
    anthropic: [
      { value: "claude-haiku-4-5-20251001", label: "Claude Haiku 4.5", recommended: true },
      { value: "claude-sonnet-4-20250514", label: "Claude Sonnet 4" },
    ],
    openai: [
      { value: "gpt-4o-mini", label: "GPT-4o Mini", recommended: true },
      { value: "gpt-4o", label: "GPT-4o" },
    ],
    deepseek: [{ value: "deepseek-chat", label: "DeepSeek Chat", recommended: true }],
    gemini: [
      { value: "gemini-2.0-flash", label: "Gemini 2.0 Flash", recommended: true },
      { value: "gemini-2.0-pro", label: "Gemini 2.0 Pro" },
    ],
    groq: [{ value: "llama-3.3-70b-versatile", label: "Llama 3.3 70B", recommended: true }],
    zhipu: [{ value: "glm-4-flash", label: "GLM-4 Flash", recommended: true }],
    dashscope: [{ value: "qwen-plus", label: "Qwen Plus", recommended: true }],
    moonshot: [{ value: "moonshot-v1-8k", label: "Moonshot V1 8K", recommended: true }],
    minimax: [{ value: "abab6.5s-chat", label: "ABAB 6.5s", recommended: true }],
  };

// ── Types ────────────────────────────────────────────────────────────

interface ProvidersData {
  [key: string]: { apiKey?: string; apiBase?: string };
}

interface AgentsData {
  defaults?: { provider?: string; model?: string };
}

interface CognitiveData {
  provider?: string;
  model?: string;
  temperature?: number;
  maxTokens?: number;
  reflectionMaxTokens?: number;
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

// ── Component ────────────────────────────────────────────────────────

export function PersonalizationSettings() {
  const toast = useToastContext();
  const { data: providers, refetch: refetchProviders } = useQuery<ProvidersData>(
    "config_get_section",
    { section: "providers" },
    {},
  );

  const { data: agents, refetch: refetchAgents } = useQuery<AgentsData>(
    "config_get_section",
    { section: "agents" },
    { defaults: {} },
  );

  const { data: learning, refetch: refetchLearning } = useQuery<LearningData>(
    "config_get_section",
    { section: "learning" },
    { enabled: true },
  );

  const { data: cognitive, refetch: refetchCognitive } = useQuery<CognitiveData>(
    "config_get_section",
    { section: "cognitive" },
    {},
  );

  const { data: providerManager, refetch: refetchProviderManager } = useQuery<ProviderManagerData>(
    "config_get_section",
    { section: "providerManager" },
    {},
  );

  // ── Provider state ───────────────────────────────────────────────

  const activeProvider = agents.defaults?.provider ?? "";
  const activeModel = agents.defaults?.model ?? "";

  const [providerEdits, setProviderEdits] = useState<Record<string, unknown>>({});
  const [savingProvider, setSavingProvider] = useState(false);

  const editedProvider = (
    "provider" in providerEdits ? providerEdits.provider : activeProvider
  ) as string;
  const editedModel = ("model" in providerEdits ? providerEdits.model : activeModel) as string;
  const editedApiKey = (
    "apiKey" in providerEdits ? providerEdits.apiKey : (providers[editedProvider]?.apiKey ?? "")
  ) as string;
  const editedApiBase = (
    "apiBase" in providerEdits ? providerEdits.apiBase : (providers[editedProvider]?.apiBase ?? "")
  ) as string;

  const hasProviderChanges = Object.keys(providerEdits).length > 0;

  const handleProviderChange = (newProvider: string) => {
    setProviderEdits((prev) => ({
      ...prev,
      provider: newProvider,
      // Reset key/base when switching providers
      apiKey: providers[newProvider]?.apiKey ?? "",
      apiBase: providers[newProvider]?.apiBase ?? "",
    }));
  };

  const saveProvider = async () => {
    setSavingProvider(true);
    try {
      // Save provider API key/base if edited
      if ("apiKey" in providerEdits || "apiBase" in providerEdits) {
        const providerPatch: Record<string, unknown> = {};
        if ("apiKey" in providerEdits) providerPatch.apiKey = providerEdits.apiKey;
        if ("apiBase" in providerEdits) {
          providerPatch.apiBase = (providerEdits.apiBase as string) || null;
        }
        await ipc("config_update_section", {
          section: "providers",
          patch: { [editedProvider]: providerPatch },
        });
      }

      // Save agent defaults if provider/model changed (sequential to avoid race)
      if ("provider" in providerEdits || "model" in providerEdits) {
        const agentsPatch: Record<string, unknown> = {};
        if ("provider" in providerEdits) agentsPatch.provider = providerEdits.provider;
        if ("model" in providerEdits) agentsPatch.model = providerEdits.model;
        await ipc("config_update_section", {
          section: "agents",
          patch: { defaults: agentsPatch },
        });
      }

      refetchProviders();
      refetchAgents();
      setProviderEdits({});
    } catch {
      toast.show("Failed to save provider config");
    } finally {
      setSavingProvider(false);
    }
  };

  // ── Learning state ───────────────────────────────────────────────

  const [learningEdits, setLearningEdits] = useState<Record<string, unknown>>({});
  const [savingLearning, setSavingLearning] = useState(false);

  const getLearningValue = <T,>(key: string, fallback: T): T => {
    if (key in learningEdits) return learningEdits[key] as T;
    const val = (learning as Record<string, unknown>)[key];
    return (val !== undefined ? val : fallback) as T;
  };

  const hasLearningChanges = Object.keys(learningEdits).length > 0;

  const saveLearning = async () => {
    setSavingLearning(true);
    try {
      await ipc("config_update_section", {
        section: "learning",
        patch: learningEdits,
      });
      refetchLearning();
      setLearningEdits({});
    } catch {
      toast.show("Failed to save learning config");
    } finally {
      setSavingLearning(false);
    }
  };

  // ── Cognitive state ──────────────────────────────────────────────

  const [cognitiveEdits, setCognitiveEdits] = useState<Record<string, unknown>>({});
  const [savingCognitive, setSavingCognitive] = useState(false);

  const cogProvider = (
    "provider" in cognitiveEdits ? cognitiveEdits.provider : (cognitive.provider ?? "")
  ) as string;
  const cogModel = (
    "model" in cognitiveEdits ? cognitiveEdits.model : (cognitive.model ?? "")
  ) as string;

  // Resolve which provider to show models for: explicit cognitive provider, or fall back to main agent provider
  const effectiveCogProvider = cogProvider || editedProvider;
  const cogModelOptions = COGNITIVE_MODELS[effectiveCogProvider] ?? [];

  const hasCognitiveChanges = Object.keys(cognitiveEdits).length > 0;

  const saveCognitive = async () => {
    setSavingCognitive(true);
    try {
      const patch: Record<string, unknown> = {};
      if ("provider" in cognitiveEdits) patch.provider = cognitiveEdits.provider || null;
      if ("model" in cognitiveEdits) patch.model = cognitiveEdits.model || null;
      if ("temperature" in cognitiveEdits) patch.temperature = cognitiveEdits.temperature;
      if ("maxTokens" in cognitiveEdits) patch.maxTokens = cognitiveEdits.maxTokens;
      if ("atomExtraction.enabled" in cognitiveEdits) {
        patch.atomExtraction = { enabled: cognitiveEdits["atomExtraction.enabled"] };
      }
      if ("intelligenceMode" in cognitiveEdits) {
        patch.intelligenceMode = cognitiveEdits.intelligenceMode;
      }
      await ipc("config_update_section", { section: "cognitive", patch });
      refetchCognitive();
      setCognitiveEdits({});
    } catch {
      toast.show("Failed to save cognitive config");
    } finally {
      setSavingCognitive(false);
    }
  };

  // ── Provider Manager state ──────────────────────────────────────

  const [pmEdits, setPmEdits] = useState<Record<string, unknown>>({});
  const [savingPm, setSavingPm] = useState(false);

  const pmVal = (key: string): string => {
    if (key in pmEdits) return (pmEdits[key] ?? "") as string;
    return ((providerManager as Record<string, unknown>)[key] ?? "") as string;
  };

  const hasPmChanges = Object.keys(pmEdits).length > 0;

  const saveProviderManager = async () => {
    setSavingPm(true);
    try {
      const patch: Record<string, unknown> = {};
      for (const [k, v] of Object.entries(pmEdits)) {
        patch[k] = v || null;
      }
      await ipc("config_update_section", { section: "providerManager", patch });
      setPmEdits({});
      refetchProviderManager();
    } catch {
      toast.show("Failed to save provider routing config");
    } finally {
      setSavingPm(false);
    }
  };

  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-foreground">Personalization</h2>
        <p className="text-[13px] text-muted-foreground mt-1">
          Provider, model, and learning preferences
        </p>
      </div>

      <div className="space-y-4">
        {/* ── Theme ──────────────────────────────────────────── */}
        <SettingsCard title="Theme">
          <ThemeSwitcher />
        </SettingsCard>

        {/* ── Provider & Model ─────────────────────────────────── */}
        <SettingsCard title="Provider & Model">
          <div className="space-y-3">
            <label className="block">
              <span className="block text-[11px] text-muted-foreground mb-1">Active provider</span>
              <select
                value={editedProvider}
                onChange={(e) => handleProviderChange(e.target.value)}
                className="w-full px-3 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors"
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

            {editedProvider && (
              <>
                <div>
                  <span className="block text-[11px] text-muted-foreground mb-1">
                    API Key ({PROVIDERS.find((p) => p.value === editedProvider)?.label})
                  </span>
                  <SecretInput
                    value={editedApiKey}
                    onChange={(v) => setProviderEdits((prev) => ({ ...prev, apiKey: v }))}
                    placeholder={`${editedProvider} API key`}
                  />
                </div>

                <label className="block">
                  <span className="block text-[11px] text-muted-foreground mb-1">
                    API Base (optional)
                  </span>
                  <input
                    type="text"
                    value={editedApiBase}
                    onChange={(e) =>
                      setProviderEdits((prev) => ({ ...prev, apiBase: e.target.value }))
                    }
                    placeholder="Leave blank for default"
                    className="w-full px-3 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
                  />
                </label>
              </>
            )}

            <label className="block">
              <span className="block text-[11px] text-muted-foreground mb-1">Default model</span>
              <input
                type="text"
                value={editedModel}
                onChange={(e) => setProviderEdits((prev) => ({ ...prev, model: e.target.value }))}
                placeholder="e.g. anthropic/claude-opus-4-5"
                className="w-full px-3 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
              />
            </label>

            {hasProviderChanges && <SaveButton onClick={saveProvider} saving={savingProvider} />}
          </div>
        </SettingsCard>

        {/* ── Cognitive AI ──────────────────────────────────────── */}
        <SettingsCard title="Cognitive AI">
          <div className="space-y-3">
            <p className="text-[11px] text-dim -mt-1">
              Background AI for memory extraction, consolidation, reflection, and coaching.
              Lightweight tasks — fast, cheaper models recommended.
            </p>

            <label className="block">
              <span className="block text-[11px] text-muted-foreground mb-1">
                Provider override
              </span>
              <select
                value={cogProvider}
                onChange={(e) =>
                  setCognitiveEdits((prev) => ({
                    ...prev,
                    provider: e.target.value,
                    // Reset model when switching provider
                    model: "",
                  }))
                }
                className="w-full px-3 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors"
              >
                <option value="" className="bg-popover">
                  Same as main ({PROVIDERS.find((p) => p.value === editedProvider)?.label || "auto"}
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
                  onChange={(e) =>
                    setCognitiveEdits((prev) => ({ ...prev, model: e.target.value }))
                  }
                  className="w-full px-3 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors"
                >
                  <option value="" className="bg-popover">
                    Same as main agent model
                  </option>
                  {cogModelOptions.map((m) => (
                    <option key={m.value} value={m.value} className="bg-popover">
                      {m.label}
                      {m.recommended ? " ★ recommended" : ""}
                    </option>
                  ))}
                </select>
              ) : (
                <input
                  type="text"
                  value={cogModel}
                  onChange={(e) =>
                    setCognitiveEdits((prev) => ({ ...prev, model: e.target.value }))
                  }
                  placeholder="Leave blank to use main agent model"
                  className="w-full px-3 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
                />
              )}
            </div>

            <div className="flex items-center justify-between pt-1 border-t border-border">
              <div>
                <span className="text-xs text-muted-foreground">Deep Intelligence Mode</span>
                <p className="text-[11px] text-dim">
                  Use full LLM processing for memory extraction and consolidation instead of
                  heuristics. Higher quality but uses more tokens.
                </p>
              </div>
              <Toggle
                checked={
                  "intelligenceMode" in cognitiveEdits
                    ? cognitiveEdits.intelligenceMode === "deep"
                    : (cognitive as CognitiveData & { intelligenceMode?: string })
                        .intelligenceMode === "deep"
                }
                onChange={(v) =>
                  setCognitiveEdits((prev) => ({
                    ...prev,
                    intelligenceMode: v ? "deep" : "standard",
                  }))
                }
              />
            </div>

            <div className="flex items-center justify-between pt-1 border-t border-border">
              <div>
                <span className="text-xs text-muted-foreground">Auto-extract knowledge atoms</span>
                <p className="text-[11px] text-dim">
                  Automatically extract concepts and facts from notes
                </p>
              </div>
              <Toggle
                checked={
                  "atomExtraction.enabled" in cognitiveEdits
                    ? (cognitiveEdits["atomExtraction.enabled"] as boolean)
                    : (cognitive.atomExtraction?.enabled ?? true)
                }
                onChange={(v) =>
                  setCognitiveEdits((prev) => ({ ...prev, "atomExtraction.enabled": v }))
                }
              />
            </div>

            {hasCognitiveChanges && (
              <>
                <p className="text-2xs text-warning/80">Changes take effect after restart</p>
                <SaveButton onClick={saveCognitive} saving={savingCognitive} />
              </>
            )}
          </div>
        </SettingsCard>

        {/* ── Learning ─────────────────────────────────────────── */}
        <SettingsCard title="Learning">
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <div>
                <span className="text-xs text-muted-foreground">Enable learning</span>
                <p className="text-[11px] text-dim">
                  Adaptive confidence thresholds based on outcomes
                </p>
              </div>
              <Toggle
                checked={getLearningValue("enabled", true)}
                onChange={(v) => setLearningEdits((prev) => ({ ...prev, enabled: v }))}
              />
            </div>

            <label className="block">
              <span className="block text-[11px] text-muted-foreground mb-1">
                Analysis interval (seconds)
              </span>
              <input
                type="number"
                value={getLearningValue("analysisIntervalSecs", 3600)}
                onChange={(e) =>
                  setLearningEdits((prev) => ({
                    ...prev,
                    analysisIntervalSecs: Number.parseInt(e.target.value, 10) || 3600,
                  }))
                }
                step="60"
                min="60"
                className="w-full px-3 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors"
              />
            </label>

            <div className="flex gap-3">
              <label className="flex-1">
                <span className="block text-[11px] text-muted-foreground mb-1">Min threshold</span>
                <input
                  type="number"
                  value={getLearningValue("minThreshold", 0.4)}
                  onChange={(e) =>
                    setLearningEdits((prev) => ({
                      ...prev,
                      minThreshold: Number.parseFloat(e.target.value) || 0.4,
                    }))
                  }
                  step="0.05"
                  min="0"
                  max="1"
                  className="w-full px-3 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors"
                />
              </label>
              <label className="flex-1">
                <span className="block text-[11px] text-muted-foreground mb-1">Max threshold</span>
                <input
                  type="number"
                  value={getLearningValue("maxThreshold", 0.9)}
                  onChange={(e) =>
                    setLearningEdits((prev) => ({
                      ...prev,
                      maxThreshold: Number.parseFloat(e.target.value) || 0.9,
                    }))
                  }
                  step="0.05"
                  min="0"
                  max="1"
                  className="w-full px-3 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors"
                />
              </label>
            </div>

            <label className="block">
              <span className="block text-[11px] text-muted-foreground mb-1">
                Min outcomes for adaptation
              </span>
              <input
                type="number"
                value={getLearningValue("minOutcomesForAdaptation", 50)}
                onChange={(e) =>
                  setLearningEdits((prev) => ({
                    ...prev,
                    minOutcomesForAdaptation: Number.parseInt(e.target.value, 10) || 50,
                  }))
                }
                min="1"
                className="w-full px-3 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors"
              />
            </label>

            {hasLearningChanges && <SaveButton onClick={saveLearning} saving={savingLearning} />}
          </div>
        </SettingsCard>

        {/* ── Provider Routing ──────────────────────────────────── */}
        <SettingsCard title="Provider routing">
          <div className="space-y-3">
            <p className="text-[11px] text-dim">
              Configure primary and fallback providers for automatic failover
            </p>

            <label className="block">
              <span className="block text-[11px] text-muted-foreground mb-1">Primary provider</span>
              <select
                value={pmVal("primary")}
                onChange={(e) => setPmEdits((prev) => ({ ...prev, primary: e.target.value }))}
                className="w-full px-3 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors"
              >
                <option value="" className="bg-popover">
                  Auto (use agent default)
                </option>
                {PROVIDERS.map((p) => (
                  <option key={p.value} value={p.value} className="bg-popover">
                    {p.label}
                  </option>
                ))}
              </select>
            </label>

            <label className="block">
              <span className="block text-[11px] text-muted-foreground mb-1">
                Fallback provider
              </span>
              <select
                value={pmVal("fallback")}
                onChange={(e) => setPmEdits((prev) => ({ ...prev, fallback: e.target.value }))}
                className="w-full px-3 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors"
              >
                <option value="" className="bg-popover">
                  None
                </option>
                {PROVIDERS.map((p) => (
                  <option key={p.value} value={p.value} className="bg-popover">
                    {p.label}
                  </option>
                ))}
              </select>
            </label>

            <label className="block">
              <span className="block text-[11px] text-muted-foreground mb-1">Classifier model</span>
              <p className="text-[11px] text-dim mb-1">
                Model used to classify request complexity for routing decisions
              </p>
              <input
                type="text"
                value={pmVal("classifierModel")}
                onChange={(e) =>
                  setPmEdits((prev) => ({ ...prev, classifierModel: e.target.value }))
                }
                placeholder="e.g. claude-haiku"
                className="w-full px-3 py-1.5 text-xs text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
              />
            </label>

            {hasPmChanges && <SaveButton onClick={saveProviderManager} saving={savingPm} />}
          </div>
        </SettingsCard>
      </div>
    </div>
  );
}
