// Hand-mirrored TS types per config section.
// Cross-check names against crates/config/src/schema/*.rs and bindings.ts.

export type ProviderConfig = {
  apiKey?: string | null;
  apiBase?: string | null;
  native?: boolean;
  cacheSystemPrompt?: boolean;
  extendedThinking?: {
    enabled: boolean;
    budgetTokens: number;
    useFor: string[];
  } | null;
  apiVersion?: string | null;
};

export type ProvidersConfig = {
  anthropic?: ProviderConfig;
  openai?: ProviderConfig;
  openrouter?: ProviderConfig;
  deepseek?: ProviderConfig;
  gemini?: ProviderConfig;
  groq?: ProviderConfig;
  vllm?: ProviderConfig;
  zhipu?: ProviderConfig;
  dashscope?: ProviderConfig;
  moonshot?: ProviderConfig;
  minimax?: ProviderConfig;
  aihubmix?: ProviderConfig;
  mimo?: ProviderConfig;
  cache?: { enabled?: boolean };
};

export type AgentsConfig = {
  defaults?: {
    workspace?: string;
    model?: string;
    provider?: string | null;
    maxTokens?: number;
    temperature?: number;
    maxToolIterations?: number;
    maxConcurrentSubagents?: number;
    execution?: {
      safetyTimeoutSecs?: number;
      adaptiveDepth?: boolean;
    };
  };
  monthlyBudgetUsd?: number | null;
  skillsDir?: string | null;
  rewriterModel?: string | null;
};

export type ProviderManagerConfig = {
  primary?: string | null;
  fallback?: string | null;
  classifierModel?: string | null;
};

export type ThemePreference = "system" | "light" | "dark" | "dim";

export type UiConfig = {
  theme?: ThemePreference;
  uiScale?: number;
  uiFontFamily?: string;
  codeFontFamily?: string;
  codeFontSize?: number;
  notificationSoundsEnabled?: boolean;
  systemNotificationsEnabled?: boolean;
  subagentSystemNotificationsEnabled?: boolean;
  threadTitleAutogenerationEnabled?: boolean;
  automaticAppUpdateChecksEnabled?: boolean;
  chatHistoryScrollbackItems?: number | null;
  showMessageFilePath?: boolean;
  splitChatDiffView?: boolean;
  usageShowRemaining?: boolean;
};
