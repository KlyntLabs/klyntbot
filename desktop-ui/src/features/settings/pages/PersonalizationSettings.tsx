import { SettingsCard } from "@shared/composites";
import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import { useToastContext } from "@shared/hooks/useToast";
import { SaveButton, SecretInput } from "@shared/ui";
import { useState } from "react";
import { ThemeSwitcher } from "../components/ThemeSwitcher";

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

interface ProvidersData {
  [key: string]: { apiKey?: string; apiBase?: string };
}

interface AgentsData {
  defaults?: { provider?: string; model?: string };
}

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
      apiKey: providers[newProvider]?.apiKey ?? "",
      apiBase: providers[newProvider]?.apiBase ?? "",
    }));
  };

  const saveProvider = async () => {
    setSavingProvider(true);
    try {
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

  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-foreground">Personalization</h2>
        <p className="text-[13px] text-muted-foreground mt-1">Theme, provider, and API keys</p>
      </div>

      <div className="space-y-4">
        <SettingsCard title="Theme">
          <ThemeSwitcher />
        </SettingsCard>

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
      </div>
    </div>
  );
}
