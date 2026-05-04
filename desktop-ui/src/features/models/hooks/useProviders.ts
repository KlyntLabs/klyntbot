import { useEffect, useState } from "react";
import { invoke } from "@/api/client";
import { PROVIDER_DISPLAY_NAMES } from "@/features/models/utils/deriveProvider";

export type ProviderInfo = {
  id: string;
  displayName: string;
  hasApiKey: boolean;
};

type ProvidersConfig = Record<string, { apiKey?: string }>;

export function useProviders() {
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const config = (await invoke("config_get_section", {
          section: "providers",
        })) as ProvidersConfig;

        if (cancelled) return;

        const list: ProviderInfo[] = Object.entries(config)
          .filter(([_, cfg]) => cfg.apiKey && cfg.apiKey.trim().length > 0)
          .map(([id]) => ({
            id,
            displayName: PROVIDER_DISPLAY_NAMES[id] ?? id,
            hasApiKey: true,
          }));

        setProviders(list);
      } catch (e) {
        console.warn("useProviders: failed to fetch providers config", e);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  return { providers, loading };
}
