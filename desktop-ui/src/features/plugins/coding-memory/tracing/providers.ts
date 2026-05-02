export interface ProviderChrome {
  id: string;
  displayName: string;
  /** CSS color value (variable or hex) for chip + header accents. */
  accent: string;
}

const REGISTRY: Record<string, ProviderChrome> = {
  kimi: { id: "kimi", displayName: "Kimi", accent: "var(--accent-violet, #7c5cff)" },
  // Future: claudeCode, codex, openCode, klyntCli.
};

export function chromeFor(providerId: string): ProviderChrome {
  return (
    REGISTRY[providerId] ?? {
      id: providerId,
      displayName: providerId,
      accent: "var(--accent-blue)",
    }
  );
}
