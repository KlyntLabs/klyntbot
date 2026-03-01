export const PROVIDER_MODELS: Record<string, string[]> = {
  anthropic: ['claude-opus-4-0520', 'claude-sonnet-4-20250514', 'claude-haiku-4-5-20251001'],
  openai: ['gpt-4o', 'gpt-4o-mini', 'gpt-4-turbo', 'o1', 'o1-mini', 'o3-mini'],
  openrouter: ['auto'],
  deepseek: ['deepseek-chat', 'deepseek-reasoner'],
  gemini: ['gemini-2.5-pro', 'gemini-2.5-flash', 'gemini-2.0-flash'],
  groq: ['llama-3.3-70b-versatile', 'llama-3.1-8b-instant', 'mixtral-8x7b-32768'],
  vllm: [],
  zhipu: ['glm-4-plus', 'glm-4-flash'],
  dashscope: ['qwen-turbo', 'qwen-plus', 'qwen-max'],
  moonshot: ['moonshot-v1-8k', 'moonshot-v1-32k'],
  minimax: ['abab6.5s-chat', 'abab5.5-chat'],
  aihubmix: [],
};

export const DISPLAY_NAMES: Record<string, string> = {
  anthropic: 'Anthropic', openai: 'OpenAI', openrouter: 'OpenRouter',
  deepseek: 'DeepSeek', gemini: 'Gemini', groq: 'Groq', vllm: 'vLLM',
  zhipu: 'Zhipu', dashscope: 'DashScope', moonshot: 'Moonshot',
  minimax: 'MiniMax', aihubmix: 'AIHubMix',
  telegram: 'Telegram', discord: 'Discord', whatsapp: 'WhatsApp',
  slack: 'Slack', email: 'Email', qq: 'QQ', feishu: 'Feishu',
  dingtalk: 'DingTalk', mochat: 'Mochat',
};

export function displayName(key: string): string {
  return DISPLAY_NAMES[key] ?? key;
}

export const TIMEZONES = [
  'UTC', 'America/New_York', 'America/Chicago', 'America/Denver',
  'America/Los_Angeles', 'America/Sao_Paulo', 'Europe/London',
  'Europe/Paris', 'Europe/Berlin', 'Europe/Moscow', 'Asia/Dubai',
  'Asia/Kolkata', 'Asia/Bangkok', 'Asia/Ho_Chi_Minh', 'Asia/Shanghai',
  'Asia/Tokyo', 'Asia/Seoul', 'Asia/Singapore', 'Australia/Sydney',
  'Pacific/Auckland',
];

export const CURRENCIES = ['USD', 'VND', 'EUR', 'GBP', 'JPY', 'KRW', 'CNY', 'THB', 'BTC', 'ETH', 'USDT'];
export const CURRENCY_SYMBOLS: Record<string, string> = {
  USD: '$', EUR: '€', GBP: '£', JPY: '¥', KRW: '₩', VND: '₫', CNY: '¥', THB: '฿', BTC: '₿',
};
