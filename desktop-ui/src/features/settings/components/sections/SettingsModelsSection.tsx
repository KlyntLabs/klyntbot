import {
  NumberField,
  SecretField,
  SelectField,
  SliderField,
  TextField,
} from "@settings/components/fields";
import type { AgentsConfig, ProvidersConfig } from "@settings/lib/configSectionTypes";
import { useConfigSection } from "@settings/lib/useConfigSection";
import { useCallback, useEffect, useState } from "react";

const PROVIDER_NAMES = [
  { value: "", label: "Auto-detect" },
  { value: "anthropic", label: "Anthropic" },
  { value: "openai", label: "OpenAI" },
  { value: "openrouter", label: "OpenRouter" },
  { value: "deepseek", label: "DeepSeek" },
  { value: "gemini", label: "Gemini" },
  { value: "groq", label: "Groq" },
  { value: "vllm", label: "vLLM" },
  { value: "zhipu", label: "Zhipu" },
  { value: "dashscope", label: "DashScope" },
  { value: "moonshot", label: "Moonshot" },
  { value: "minimax", label: "MiniMax" },
  { value: "aihubmix", label: "AI Hub Mix" },
  { value: "mimo", label: "Mimo" },
];

const PROVIDER_KEYS = [
  "anthropic",
  "openai",
  "openrouter",
  "deepseek",
  "gemini",
  "groq",
  "vllm",
  "zhipu",
  "dashscope",
  "moonshot",
  "minimax",
  "aihubmix",
  "mimo",
] as const;

export function SettingsModelsSection() {
  const {
    value: agents,
    loading: agentsLoading,
    error: agentsError,
    patch: patchAgents,
  } = useConfigSection<AgentsConfig>("agents");

  const {
    value: providers,
    loading: providersLoading,
    error: providersError,
    patch: patchProviders,
  } = useConfigSection<ProvidersConfig>("providers");

  const [modelDraft, setModelDraft] = useState("");
  const [providerDraft, setProviderDraft] = useState("");
  const [temperatureDraft, setTemperatureDraft] = useState(0.7);
  const [maxTokensDraft, setMaxTokensDraft] = useState(8192);
  const [maxToolIterationsDraft, setMaxToolIterationsDraft] = useState(20);
  const [budgetDraft, setBudgetDraft] = useState<number | null>(null);

  useEffect(() => {
    if (agents) {
      setModelDraft(agents.defaults?.model ?? "");
      setProviderDraft(agents.defaults?.provider ?? "");
      setTemperatureDraft(agents.defaults?.temperature ?? 0.7);
      setMaxTokensDraft(agents.defaults?.maxTokens ?? 8192);
      setMaxToolIterationsDraft(agents.defaults?.maxToolIterations ?? 20);
      setBudgetDraft(agents.monthlyBudgetUsd ?? null);
    }
  }, [agents]);

  const commitAgentDefaults = useCallback(
    async (patch: Partial<AgentsConfig["defaults"]>) => {
      await patchAgents({ defaults: { ...agents?.defaults, ...patch } });
    },
    [agents, patchAgents],
  );

  const handleProviderKeyChange = useCallback(
    async (providerKey: string, keyValue: string) => {
      const current = providers?.[providerKey as keyof ProvidersConfig] as
        | { apiKey?: string | null }
        | undefined;
      await patchProviders({
        [providerKey]: { ...current, apiKey: keyValue || null },
      });
    },
    [providers, patchProviders],
  );

  const isLoading = agentsLoading || providersLoading;
  const error = agentsError || providersError;

  return (
    <div className="flex flex-col gap-2">
      <h2 className="text-[var(--fs-lg)] font-semibold text-[var(--text-strong)]">
        Models &amp; Providers
      </h2>

      {isLoading && <div className="text-[var(--fs-sm)] text-[var(--text-subtle)]">Loading…</div>}

      {error && (
        <div className="rounded-lg border border-red-400/30 bg-red-400/10 p-4 text-[var(--fs-sm)] text-red-400">
          {error}
        </div>
      )}

      <div className="rounded-lg border border-[var(--border-subtle)] p-4">
        <h3 className="mb-2 text-[var(--fs-md)] font-medium text-[var(--text-strong)]">Defaults</h3>
        <span className="mb-3 block text-[var(--fs-xs)] text-[var(--text-subtle)]">
          Changes apply immediately to new assistant turns.
        </span>

        <TextField
          label="Model"
          description="e.g. anthropic/claude-opus-4-5"
          value={modelDraft}
          onChange={setModelDraft}
          onBlur={() => commitAgentDefaults({ model: modelDraft })}
        />

        <SelectField
          label="Provider"
          description="Overrides auto-detection"
          value={providerDraft}
          options={PROVIDER_NAMES}
          onChange={(v) => {
            setProviderDraft(v);
            commitAgentDefaults({ provider: v || null });
          }}
        />

        <SliderField
          label="Temperature"
          value={temperatureDraft}
          min={0}
          max={2}
          step={0.05}
          onChange={(v) => setTemperatureDraft(v)}
          onBlur={() => commitAgentDefaults({ temperature: temperatureDraft })}
        />

        <NumberField
          label="Max tokens"
          value={maxTokensDraft}
          min={1}
          max={128000}
          step={1}
          onChange={setMaxTokensDraft}
          onBlur={() => commitAgentDefaults({ maxTokens: maxTokensDraft })}
        />

        <NumberField
          label="Max tool iterations"
          value={maxToolIterationsDraft}
          min={1}
          max={100}
          step={1}
          onChange={setMaxToolIterationsDraft}
          onBlur={() => commitAgentDefaults({ maxToolIterations: maxToolIterationsDraft })}
        />

        <NumberField
          label="Monthly budget (USD)"
          description="0 = unlimited"
          value={budgetDraft ?? 0}
          min={0}
          step={1}
          onChange={(v) => setBudgetDraft(v)}
          onBlur={() => patchAgents({ monthlyBudgetUsd: budgetDraft || null })}
        />
      </div>

      <div className="rounded-lg border border-[var(--border-subtle)] p-4">
        <h3 className="mb-2 text-[var(--fs-md)] font-medium text-[var(--text-strong)]">API Keys</h3>
        {PROVIDER_KEYS.map((key) => {
          const cfg = providers?.[key as keyof ProvidersConfig] as
            | { apiKey?: string | null }
            | undefined;
          const hasKey = !!(cfg?.apiKey && cfg.apiKey.length > 0);
          return (
            <SecretField
              key={key}
              label={key.charAt(0).toUpperCase() + key.slice(1)}
              configured={hasKey}
              onChange={(v) => handleProviderKeyChange(key, v)}
            />
          );
        })}
      </div>
    </div>
  );
}
