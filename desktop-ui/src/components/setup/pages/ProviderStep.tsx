import { Eye, EyeOff } from "lucide-react";
import { useId, useState } from "react";
import { useOutletContext } from "react-router";
import { ipc } from "../../../hooks/useIpc";
import type { SetupContext } from "../steps";

const PROVIDERS = [
  { value: "anthropic", label: "Anthropic", defaultModel: "anthropic/claude-opus-4-5" },
  { value: "openai", label: "OpenAI", defaultModel: "openai/gpt-4o" },
  {
    value: "openrouter",
    label: "OpenRouter",
    defaultModel: "openrouter/anthropic/claude-3.5-sonnet",
  },
  { value: "deepseek", label: "DeepSeek", defaultModel: "deepseek/deepseek-chat" },
  { value: "gemini", label: "Google Gemini", defaultModel: "gemini/gemini-2.0-flash" },
  { value: "groq", label: "Groq", defaultModel: "groq/llama-3.3-70b-versatile" },
] as const;

export function ProviderStep() {
  const { next } = useOutletContext<SetupContext>();
  const id = useId();

  const [provider, setProvider] = useState("anthropic");
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState(PROVIDERS[0].defaultModel);
  const [showKey, setShowKey] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleProviderChange = (value: string) => {
    setProvider(value);
    const match = PROVIDERS.find((p) => p.value === value);
    if (match) setModel(match.defaultModel);
  };

  const selectedProvider = PROVIDERS.find((p) => p.value === provider);

  const handleContinue = async () => {
    if (!apiKey.trim()) {
      setError("API key is required");
      return;
    }

    setSaving(true);
    setError(null);

    try {
      // Sequential to avoid race — both write to the same config file
      await ipc("config_update_section", {
        section: "providers",
        patch: { [provider]: { apiKey: apiKey.trim() } },
      });
      await ipc("config_update_section", {
        section: "agents",
        patch: { defaults: { model, provider } },
      });

      next();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to save provider config");
    } finally {
      setSaving(false);
    }
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
          <div className="relative">
            <input
              id={`${id}-apikey`}
              type={showKey ? "text" : "password"}
              value={apiKey}
              onChange={(e) => {
                setApiKey(e.target.value);
                setError(null);
              }}
              placeholder={`Enter your ${selectedProvider?.label} API key`}
              className="w-full px-3 py-2 pr-10 text-[13px] text-primary bg-surface-base border border-border rounded-lg focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
            />
            <button
              type="button"
              onClick={() => setShowKey(!showKey)}
              className="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted hover:text-secondary transition-colors"
            >
              {showKey ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
            </button>
          </div>
        </div>

        <div>
          <label
            htmlFor={`${id}-model`}
            className="block text-[12px] font-medium text-secondary mb-1.5"
          >
            Model
          </label>
          <input
            id={`${id}-model`}
            type="text"
            value={model}
            onChange={(e) => setModel(e.target.value)}
            className="w-full px-3 py-2 text-[13px] text-primary bg-surface-base border border-border rounded-lg focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
          />
          <p className="text-[11px] text-dim mt-1">
            Default model for all agents. Can be overridden per-agent later.
          </p>
        </div>

        {error && <p className="text-[12px] text-destructive">{error}</p>}
      </div>

      <div className="mt-6 flex justify-end">
        <button
          type="button"
          onClick={handleContinue}
          disabled={saving}
          className="px-5 py-2 text-[13px] font-medium text-white bg-brand hover:bg-brand-hover rounded-xl transition-colors disabled:opacity-50"
        >
          {saving ? "Saving..." : "Continue"}
        </button>
      </div>
    </div>
  );
}
