import { useCallback, useEffect, useState } from "react";
import { invoke } from "@/api/client";
import { PROVIDER_DISPLAY_NAMES } from "@/features/models/utils/deriveProvider";

/// A provider entry surfaced to the FE meta bar.
///
/// `id` is the *brand* (e.g. `moonshot`, `anthropic`, `deepseek`) — the
/// thing the user thinks they're talking to. The model list filter
/// matches on `model.brand === id`. The actual transport adapter (the
/// "config-provider key") is implicit in the model row and is what the
/// backend routes through.
export type ProviderInfo = {
  id: string;
  displayName: string;
  hasApiKey: boolean;
};

type ProvidersConfig = Record<string, { apiKey?: string }>;

type AgentsConfig = { defaults?: { provider?: string | null; model?: string | null } };

type ModelEntry = {
  id: string;
  provider: string;
  brand?: string | null;
  isDefault?: boolean;
};

/// Build the brand-keyed provider list from the aggregated model list.
///
/// Why brand-keyed and not config-provider-keyed: a single config
/// provider can serve multiple brands (Kimi served via Anthropic
/// compat → `provider="anthropic"`, `brand="moonshot"`), and the user
/// expects the pill to say "Moonshot", not "Anthropic". This is what
/// every other AI client does (opencode, kimi-cli, claude-code) and
/// the architecture already supports it because the backend tags
/// every model with `brand`.
function brandsFromModels(models: ModelEntry[]): ProviderInfo[] {
  const seen = new Map<string, ProviderInfo>();
  for (const m of models) {
    const brand = m.brand?.trim();
    if (!brand || seen.has(brand)) continue;
    seen.set(brand, {
      id: brand,
      displayName: PROVIDER_DISPLAY_NAMES[brand] ?? brand,
      hasApiKey: true,
    });
  }
  return [...seen.values()].sort((a, b) => a.displayName.localeCompare(b.displayName));
}

function hasConfiguredProvider(providersCfg: ProvidersConfig): boolean {
  return Object.entries(providersCfg).some(
    ([_, cfg]) =>
      cfg && typeof cfg === "object" && "apiKey" in cfg && cfg.apiKey && cfg.apiKey.trim().length > 0,
  );
}

export function useProviders() {
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [defaultProviderId, setDefaultProviderId] = useState<string | null>(null);
  const [hasApiKeyConfigured, setHasApiKeyConfigured] = useState(false);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      // Pull the workspace id so model_list can be scoped (today the
      // backend ignores it, but the IPC accepts it for forward
      // compat with per-workspace overrides).
      const workspaces = (await invoke("workspaces_list").catch(() => [])) as Array<{ id: string }>;
      const workspaceId = workspaces[0]?.id ?? "";
      const [models, providersCfg, agentsCfg] = await Promise.all([
        invoke("model_list", { workspaceId }).catch(() => []) as Promise<ModelEntry[]>,
        invoke("config_get_section", { section: "providers" }) as Promise<ProvidersConfig>,
        invoke("config_get_section", { section: "agents" }) as Promise<AgentsConfig>,
      ]);

      // Primary path: derive providers from brands present in the
      // aggregated model list. Falls back to the legacy config-key
      // list when model_list is empty (e.g. no API keys configured).
      let list: ProviderInfo[] = brandsFromModels(models);
      if (list.length === 0) {
        list = Object.entries(providersCfg)
          .filter(([_, cfg]) => cfg.apiKey && cfg.apiKey.trim().length > 0)
          .map(([id]) => ({
            id,
            displayName: PROVIDER_DISPLAY_NAMES[id] ?? id,
            hasApiKey: true,
          }));
      }

      // Default provider = brand of the configured default model,
      // when discoverable. Fall back to agents.defaults.provider if
      // the model isn't in the catalogue, then to the first entry.
      const defaultModel = agentsCfg?.defaults?.model ?? null;
      const defaultBrand =
        (defaultModel && models.find((m) => m.id === defaultModel)?.brand) || null;
      const cfgDefault = agentsCfg?.defaults?.provider ?? null;
      const resolvedDefault =
        (defaultBrand && list.some((p) => p.id === defaultBrand) ? defaultBrand : null) ??
        (cfgDefault && list.some((p) => p.id === cfgDefault) ? cfgDefault : null) ??
        list[0]?.id ??
        null;

      setProviders(list);
      setDefaultProviderId(resolvedDefault);
      setHasApiKeyConfigured(hasConfiguredProvider(providersCfg));
    } catch (e) {
      console.warn("useProviders: failed to fetch providers config", e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  return { providers, defaultProviderId, hasApiKeyConfigured, loading, refresh: load };
}
