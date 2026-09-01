export const PROVIDER_DISPLAY_NAMES: Record<string, string> = {
  anthropic: "Anthropic",
  openai: "OpenAI",
  openrouter: "OpenRouter",
  deepseek: "DeepSeek",
  gemini: "Gemini",
  groq: "Groq",
  vllm: "vLLM/Local",
  zhipu: "Zhipu AI",
  dashscope: "DashScope",
  moonshot: "Moonshot",
  minimax: "MiniMax",
  aihubmix: "AiHubMix",
};

const PROVIDER_KEYWORDS: Record<string, string[]> = {
  anthropic: ["anthropic", "claude"],
  openai: ["openai", "gpt"],
  deepseek: ["deepseek"],
  gemini: ["gemini"],
  zhipu: ["zhipu", "glm", "zai"],
  dashscope: ["qwen", "dashscope"],
  moonshot: ["moonshot", "kimi"],
  minimax: ["minimax"],
  groq: ["groq"],
  vllm: ["vllm"],
  aihubmix: ["aihubmix"],
  openrouter: ["openrouter"],
};

export function deriveProviderFromModel(modelName: string): string | null {
  const lower = modelName.toLowerCase();
  for (const [provider, keywords] of Object.entries(PROVIDER_KEYWORDS)) {
    if (keywords.some((kw) => lower.includes(kw))) {
      return provider;
    }
  }
  return null;
}
