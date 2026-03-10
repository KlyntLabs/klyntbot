import { ipc } from "@shared/hooks/useIpc";
import { SecretInput } from "@shared/ui";
import { useEffect, useId, useState } from "react";
import { useOutletContext } from "react-router";
import type { SetupContext } from "../hooks/steps";

interface ProviderDef {
  value: string;
  label: string;
  models: { value: string; label: string }[];
}

const PROVIDERS: ProviderDef[] = [
  {
    value: "anthropic",
    label: "Anthropic",
    models: [
      { value: "claude-sonnet-4-20250514", label: "Claude Sonnet 4" },
      { value: "claude-opus-4-20250514", label: "Claude Opus 4" },
      { value: "claude-haiku-4-5-20251001", label: "Claude Haiku 4.5" },
    ],
  },
  {
    value: "openai",
    label: "OpenAI",
    models: [
      { value: "gpt-4o", label: "GPT-4o" },
      { value: "gpt-4o-mini", label: "GPT-4o Mini" },
      { value: "o3", label: "o3" },
      { value: "o4-mini", label: "o4 Mini" },
    ],
  },
  {
    value: "openrouter",
    label: "OpenRouter",
    models: [
      { value: "anthropic/claude-sonnet-4", label: "Claude Sonnet 4" },
      { value: "openai/gpt-4o", label: "GPT-4o" },
      { value: "google/gemini-2.5-pro", label: "Gemini 2.5 Pro" },
      { value: "deepseek/deepseek-chat", label: "DeepSeek Chat" },
    ],
  },
  {
    value: "deepseek",
    label: "DeepSeek",
    models: [
      { value: "deepseek-chat", label: "DeepSeek Chat (V3)" },
      { value: "deepseek-reasoner", label: "DeepSeek Reasoner (R1)" },
    ],
  },
  {
    value: "gemini",
    label: "Google Gemini",
    models: [
      { value: "gemini-2.5-pro", label: "Gemini 2.5 Pro" },
      { value: "gemini-2.5-flash", label: "Gemini 2.5 Flash" },
      { value: "gemini-2.0-flash", label: "Gemini 2.0 Flash" },
    ],
  },
  {
    value: "groq",
    label: "Groq",
    models: [
      { value: "llama-3.3-70b-versatile", label: "Llama 3.3 70B" },
      { value: "llama-3.1-8b-instant", label: "Llama 3.1 8B" },
      { value: "gemma2-9b-it", label: "Gemma 2 9B" },
    ],
  },
];

export function ProviderStep() {
  const { forwardRef, setDirty } = useOutletContext<SetupContext>();
  const id = useId();

  const [provider, setProvider] = useState("anthropic");
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState(PROVIDERS[0].models[0].value);
  const [error, setError] = useState<string | null>(null);

  const selectedProvider = PROVIDERS.find((p) => p.value === provider);

  // Load saved config on mount
  useEffect(() => {
    Promise.all([
      ipc<{ defaults?: { provider?: string; model?: string } }>("config_get_section", {
        section: "agents",
      }).catch(() => ({})),
      ipc<Record<string, { apiKey?: string }>>("config_get_section", {
        section: "providers",
      }).catch((): Record<string, { apiKey?: string }> => ({})),
    ]).then(([agents, providers]) => {
      const defaults = agents && "defaults" in agents ? agents.defaults : undefined;
      if (defaults?.provider) {
        setProvider(defaults.provider);
        const match = PROVIDERS.find((p) => p.value === defaults.provider);
        if (defaults.model && match?.models.some((m) => m.value === defaults.model)) {
          setModel(defaults.model);
        } else if (match) {
          setModel(match.models[0].value);
        }
        const providerConfig = providers?.[defaults.provider];
        if (providerConfig?.apiKey) {
          setApiKey(providerConfig.apiKey);
          setDirty(true);
        }
      }
    });
  }, [setDirty]);

  // Register save handler with layout
  useEffect(() => {
    forwardRef.current = async () => {
      if (!apiKey.trim()) {
        setError("API key is required");
        return false;
      }
      setError(null);
      try {
        await Promise.all([
          ipc("config_update_section", {
            section: "providers",
            patch: { [provider]: { apiKey: apiKey.trim() } },
          }),
          ipc("config_update_section", {
            section: "agents",
            patch: { defaults: { model, provider } },
          }),
        ]);
      } catch (e) {
        setError(e instanceof Error ? e.message : "Failed to save provider config");
        return false;
      }
      return true;
    };
  }, [forwardRef, provider, apiKey, model]);

  const handleProviderChange = (value: string) => {
    setProvider(value);
    const match = PROVIDERS.find((p) => p.value === value);
    if (match) setModel(match.models[0].value);
    setDirty(true);
  };

  return (
    <div>
      <h2 className="text-lg font-medium text-primary mb-1">Provider & Model</h2>
      <p className="text-[13px] text-muted mb-6">
        Choose your LLM provider and enter your API key.
      </p>

      <div className="space-y-4">
        <div>
          <label
            htmlFor={`${id}-provider`}
            className="block text-[12px] font-medium text-secondary mb-1.5"
          >
            Provider
          </label>
          <select
            id={`${id}-provider`}
            value={provider}
            onChange={(e) => handleProviderChange(e.target.value)}
            className="w-full px-3 py-2 text-[13px] text-primary bg-surface-base border border-border rounded-lg focus:outline-none focus:border-brand/50 transition-colors"
          >
            {PROVIDERS.map((p) => (
              <option key={p.value} value={p.value} className="bg-background">
                {p.label}
              </option>
            ))}
          </select>
        </div>

        <div>
          <label
            htmlFor={`${id}-apikey`}
            className="block text-[12px] font-medium text-secondary mb-1.5"
          >
            API Key
          </label>
          <SecretInput
            value={apiKey}
            onChange={(v) => {
              setApiKey(v);
              setError(null);
              setDirty(true);
            }}
            placeholder={`Enter your ${selectedProvider?.label} API key`}
            className="w-full px-3 py-2 pr-9 text-[13px] text-primary bg-surface-base border border-border rounded-lg focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
          />
        </div>

        <div>
          <label
            htmlFor={`${id}-model`}
            className="block text-[12px] font-medium text-secondary mb-1.5"
          >
            Model
          </label>
          <select
            id={`${id}-model`}
            value={model}
            onChange={(e) => {
              setModel(e.target.value);
              setDirty(true);
            }}
            className="w-full px-3 py-2 text-[13px] text-primary bg-surface-base border border-border rounded-lg focus:outline-none focus:border-brand/50 transition-colors"
          >
            {selectedProvider?.models.map((m) => (
              <option key={m.value} value={m.value} className="bg-background">
                {m.label}
              </option>
            ))}
          </select>
          <p className="text-[11px] text-dim mt-1">
            Default model for all agents. Can be overridden per-agent later.
          </p>
        </div>

        {error && <p className="text-[12px] text-destructive">{error}</p>}
      </div>
    </div>
  );
}
