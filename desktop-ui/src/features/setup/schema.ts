import { ipc } from "@shared/hooks/useIpc";

// ── Types ─────────────────────────────────────────────────────────

export type InputType =
  | "text"
  | "select"
  | "masked"
  | "confirm"
  | "tags"
  | "checkbox-list"
  | "complex";

export type NodeValue = string | boolean | string[];

export type TranscriptValues = Record<string, NodeValue>;

export interface ConversationNode {
  id: string;
  prompt: string; // Sentence template with {input} placeholder
  inputType: InputType;
  default?: NodeValue;
  options?: { label: string; value: string }[];
  checkboxOptions?: { label: string; value: string; detected?: boolean; hint?: string }[];
  validate?: (value: string) => string | null;
  condition?: (values: TranscriptValues) => boolean;
  save?: (value: NodeValue, values: TranscriptValues) => Promise<void>;
  load?: () => Promise<NodeValue | null>;
}

// ── Shared Constants ──────────────────────────────────────────────

export const AREA_COLORS = [
  "#f97316",
  "#ef4444",
  "#8b5cf6",
  "#3b82f6",
  "#06b6d4",
  "#10b981",
  "#eab308",
  "#ec4899",
];

const PROVIDER_OPTIONS = [
  { label: "Anthropic", value: "anthropic" },
  { label: "OpenAI", value: "openai" },
  { label: "OpenRouter", value: "openrouter" },
  { label: "DeepSeek", value: "deepseek" },
  { label: "Google Gemini", value: "gemini" },
  { label: "Groq", value: "groq" },
];

// ── Schema Definition ─────────────────────────────────────────────

export const CONVERSATION_SCHEMA: ConversationNode[] = [
  {
    id: "user_name",
    prompt: "Hello, I'm Klynt. Your name is {input}.",
    inputType: "text",
    validate: (v) => (v.trim() ? null : "Please enter your name"),
    save: async (value) => {
      await ipc("config_update_section", {
        section: "user",
        patch: { name: (value as string).trim() },
      });
    },
    load: async () => {
      const data = await ipc<{ name?: string }>("config_get_section", { section: "user" }).catch(
        () => null,
      );
      return data?.name || null;
    },
  },
  {
    id: "provider",
    prompt: "I'll be powered by {input}.",
    inputType: "select",
    default: "anthropic",
    options: PROVIDER_OPTIONS,
    save: async (value) => {
      await ipc("config_update_section", {
        section: "agents",
        patch: { defaults: { provider: value } },
      });
    },
    load: async () => {
      const data = await ipc<{ defaults?: { provider?: string } }>("config_get_section", {
        section: "agents",
      }).catch(() => null);
      return data?.defaults?.provider || null;
    },
  },
  {
    id: "api_key",
    prompt: "My API key is {input}.",
    inputType: "masked",
    validate: (v) => (v.trim() ? null : "API key is required"),
    save: async (value, values) => {
      const provider = (values.provider as string) || "anthropic";
      await ipc("config_update_section", {
        section: "providers",
        patch: { [provider]: { apiKey: (value as string).trim() } },
      });
    },
    load: async () => {
      // Check if any provider has a key configured
      const providers = await ipc<Record<string, { apiKey?: string }>>("config_get_section", {
        section: "providers",
      }).catch(() => null);
      if (!providers) return null;
      for (const p of Object.values(providers)) {
        if (p && typeof p === "object" && p.apiKey) return p.apiKey;
      }
      return null;
    },
  },
  {
    id: "areas",
    prompt: "Your main areas of focus are {input}.",
    inputType: "tags",
    default: ["Work", "Personal", "Health", "Learning"],
    save: async (value) => {
      const names = value as string[];
      // Load existing areas to avoid creating duplicates on re-edit
      const existing = await ipc<{ id: string; name: string }[]>("area_list").catch(() => []);
      const existingNames = new Set((existing ?? []).map((a) => a.name));
      const newNames = names.filter((n) => !existingNames.has(n.trim()));
      await Promise.all(
        newNames.map((name, i) =>
          ipc("area_create", {
            params: {
              name: name.trim(),
              color: AREA_COLORS[(existingNames.size + i) % AREA_COLORS.length],
            },
          }),
        ),
      );
    },
    load: async () => {
      const areas = await ipc<{ name: string }[]>("area_list").catch(() => null);
      if (areas && areas.length > 0) return areas.map((a) => a.name);
      return null;
    },
  },
  {
    id: "productivity_gate",
    prompt: "Would you like to enable productivity tracking? {input}",
    inputType: "confirm",
    default: true,
    save: async (value) => {
      await ipc("config_update_section", {
        section: "productivity",
        patch: { enabled: value },
      });
    },
    // No load — always shown (Rust defaults make it always appear "completed")
  },
  {
    id: "finance_gate",
    prompt: "Would you like to set up finance tracking? {input}",
    inputType: "confirm",
    default: false,
    // No save — controls flow only
    // No load — always shown
  },
  {
    id: "finance_setup",
    prompt: "",
    inputType: "complex",
    condition: (values) => values.finance_gate === true,
  },
  {
    id: "ai_tools",
    prompt: "Connect me with your AI coding tools: {input}",
    inputType: "checkbox-list",
    checkboxOptions: [], // Populated dynamically from ai_tools_detect
    save: async (value) => {
      const tools = value as string[];
      if (tools.length > 0) {
        await ipc("ai_tools_install", { params: { tools } });
      }
    },
    load: async () => {
      const config = await ipc<{ aiTools?: string[] }>("config_get_section", {
        section: "integrations",
      }).catch(() => null);
      return config?.aiTools?.length ? config.aiTools : null;
    },
  },
  {
    id: "complete",
    prompt: "Great, we're all set. Let's get started!",
    inputType: "text", // unused — no input on complete node
    save: async () => {
      await ipc("config_mark_setup_completed");
    },
  },
];
