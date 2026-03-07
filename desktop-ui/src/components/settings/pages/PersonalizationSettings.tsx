import { useState } from "react";
import { ipc } from "../../../hooks/useIpc";
import { useQuery } from "../../../hooks/useQuery";
import { SaveButton } from "../../shared/SaveButton";
import { SecretInput } from "../../shared/SecretInput";
import { SettingsCard } from "../../shared/SettingsCard";
import { Toggle } from "../../shared/Toggle";

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

// ── Types ────────────────────────────────────────────────────────────

interface ProvidersData {
  [key: string]: { apiKey?: string; apiBase?: string };
}

interface AgentsData {
  defaults?: { provider?: string; model?: string };
}

interface LearningData {
  enabled?: boolean;
  analysisIntervalSecs?: number;
  minThreshold?: number;
  maxThreshold?: number;
  minOutcomesForAdaptation?: number;
}

// ── Component ────────────────────────────────────────────────────────

export function PersonalizationSettings() {
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
    } catch (e) {
      console.error("Failed to save provider config:", e);
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
    } catch (e) {
      console.error("Failed to save learning config:", e);
    } finally {
      setSavingLearning(false);
    }
  };

  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-primary">Personalization</h2>
        <p className="text-[13px] text-muted mt-1">Provider, model, and learning preferences</p>
      </div>

      <div className="space-y-4">
        {/* ── Provider & Model ─────────────────────────────────── */}
        <SettingsCard title="Provider & Model">
          <div className="space-y-3">
            <label className="block">
              <span className="block text-[11px] text-muted mb-1">Active provider</span>
              <select
                value={editedProvider}
                onChange={(e) => handleProviderChange(e.target.value)}
                className="w-full px-3 py-1.5 text-[12px] text-primary bg-white/[0.06] border border-white/[0.08] rounded-md focus:outline-none focus:border-brand/50 transition-colors"
              >
                <option value="" className="bg-[#1a1a1a]">
                  Auto-detect
                </option>
                {PROVIDERS.map((p) => (
                  <option key={p.value} value={p.value} className="bg-[#1a1a1a]">
                    {p.label}
                  </option>
                ))}
              </select>
            </label>

            {editedProvider && (
              <>
                <div>
                  <span className="block text-[11px] text-muted mb-1">
                    API Key ({PROVIDERS.find((p) => p.value === editedProvider)?.label})
                  </span>
                  <SecretInput
                    value={editedApiKey}
                    onChange={(v) => setProviderEdits((prev) => ({ ...prev, apiKey: v }))}
                    placeholder={`${editedProvider} API key`}
                  />
                </div>

                <label className="block">
                  <span className="block text-[11px] text-muted mb-1">API Base (optional)</span>
                  <input
                    type="text"
                    value={editedApiBase}
                    onChange={(e) =>
                      setProviderEdits((prev) => ({ ...prev, apiBase: e.target.value }))
                    }
                    placeholder="Leave blank for default"
                    className="w-full px-3 py-1.5 text-[12px] text-primary bg-white/[0.06] border border-white/[0.08] rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
                  />
                </label>
              </>
            )}

            <label className="block">
              <span className="block text-[11px] text-muted mb-1">Default model</span>
              <input
                type="text"
                value={editedModel}
                onChange={(e) => setProviderEdits((prev) => ({ ...prev, model: e.target.value }))}
                placeholder="e.g. anthropic/claude-opus-4-5"
                className="w-full px-3 py-1.5 text-[12px] text-primary bg-white/[0.06] border border-white/[0.08] rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
              />
            </label>

            {hasProviderChanges && <SaveButton onClick={saveProvider} saving={savingProvider} />}
          </div>
        </SettingsCard>

        {/* ── Learning ─────────────────────────────────────────── */}
        <SettingsCard title="Learning">
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <div>
                <span className="text-[12px] text-secondary">Enable learning</span>
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
              <span className="block text-[11px] text-muted mb-1">Analysis interval (seconds)</span>
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
                className="w-full px-3 py-1.5 text-[12px] text-primary bg-white/[0.06] border border-white/[0.08] rounded-md focus:outline-none focus:border-brand/50 transition-colors"
              />
            </label>

            <div className="flex gap-3">
              <label className="flex-1">
                <span className="block text-[11px] text-muted mb-1">Min threshold</span>
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
                  className="w-full px-3 py-1.5 text-[12px] text-primary bg-white/[0.06] border border-white/[0.08] rounded-md focus:outline-none focus:border-brand/50 transition-colors"
                />
              </label>
              <label className="flex-1">
                <span className="block text-[11px] text-muted mb-1">Max threshold</span>
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
                  className="w-full px-3 py-1.5 text-[12px] text-primary bg-white/[0.06] border border-white/[0.08] rounded-md focus:outline-none focus:border-brand/50 transition-colors"
                />
              </label>
            </div>

            <label className="block">
              <span className="block text-[11px] text-muted mb-1">Min outcomes for adaptation</span>
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
                className="w-full px-3 py-1.5 text-[12px] text-primary bg-white/[0.06] border border-white/[0.08] rounded-md focus:outline-none focus:border-brand/50 transition-colors"
              />
            </label>

            {hasLearningChanges && <SaveButton onClick={saveLearning} saving={savingLearning} />}
          </div>
        </SettingsCard>
      </div>
    </div>
  );
}
