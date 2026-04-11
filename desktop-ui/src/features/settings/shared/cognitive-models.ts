// Recommended cognitive models per provider — fast & cheap models ideal for
// extraction, consolidation, reflection, coaching reasoning, and query
// enhancement subsystems.

export interface CognitiveModelOption {
  value: string;
  label: string;
  recommended?: boolean;
}

export const COGNITIVE_MODELS: Record<string, CognitiveModelOption[]> = {
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
