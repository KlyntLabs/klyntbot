# Conversational Setup Wizard Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the traditional 8-step setup wizard with a conversational onboarding flow where the AI introduces itself and learns about the user through sentence-based inline prompts.

**Architecture:** Declarative schema-driven `ConversationRunner` engine. Each onboarding question is a `ConversationNode` object in an array. The engine iterates through nodes, renders completed ones as locked text, and renders the active node with a live inline input. Save-on-Enter via Tauri IPC. Finance uses a slide-down panel wrapping existing sub-forms.

**Tech Stack:** React 19, TypeScript, Tailwind v4 (CSS token system), Tauri IPC (`ipc()` helper), existing `SecretInput` component for masked input, existing finance sub-form components.

**Spec:** `docs/superpowers/specs/2026-03-13-conversational-setup-wizard-design.md`

---

## Chunk 1: Backend — Add UserConfig to Config

### Task 1: Add UserConfig struct and wire into Config

**Files:**
- Create: `crates/config/src/schema/user.rs`
- Modify: `crates/config/src/schema/mod.rs`
- Modify: `crates/config/src/schema/core.rs`

- [ ] **Step 1: Write test for UserConfig serialization**

Add to the existing test module in `crates/config/src/schema/mod.rs`:

```rust
#[test]
fn test_user_config_default() {
    let config = Config::default();
    assert_eq!(config.user.name, "");
}

#[test]
fn test_user_config_serde_roundtrip() {
    let json = r#"{"user": {"name": "Vu"}}"#;
    let config: Config = serde_json::from_str(json).unwrap();
    assert_eq!(config.user.name, "Vu");

    let serialized = serde_json::to_string(&config).unwrap();
    let loaded: Config = serde_json::from_str(&serialized).unwrap();
    assert_eq!(loaded.user.name, "Vu");
}

#[test]
fn test_config_without_user_deserializes() {
    // Existing configs without "user" section should still deserialize
    let json = r#"{"agents": {"defaults": {"workspace": "~/.klyntbot/workspace", "model": "anthropic/claude-opus-4-5", "maxTokens": 8192, "temperature": 0.7, "maxToolIterations": 20}}}"#;
    let config: Config = serde_json::from_str(json).unwrap();
    assert_eq!(config.user.name, "");
}
```

- [ ] **Step 2: Run tests — should fail (no `user` field on Config)**

Run: `cargo nextest run -p config -E 'test(user_config)'`
Expected: compilation error — `Config` has no field named `user`

- [ ] **Step 3: Create `user.rs`**

Create `crates/config/src/schema/user.rs`:

```rust
//! User profile configuration.

use serde::{Deserialize, Serialize};

/// User profile settings collected during onboarding.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserConfig {
    #[serde(default)]
    pub name: String,
}
```

- [ ] **Step 4: Wire into mod.rs**

In `crates/config/src/schema/mod.rs`, add after the `mod work_context;` line:

```rust
mod user;
```

And after `pub use self::work_context::*;`:

```rust
pub use self::user::*;
```

- [ ] **Step 5: Add user field to Config**

In `crates/config/src/schema/core.rs`, add an explicit import (glob re-exports from `mod.rs` are NOT in scope inside sibling `core.rs`):

```rust
use super::user::UserConfig;
```

Add the field to `Config` struct after the `work_context` field:

```rust
    /// User profile settings (name, preferences).
    #[serde(default)]
    pub user: UserConfig,
```

- [ ] **Step 6: Run tests — should pass**

Run: `cargo nextest run -p config -E 'test(user_config)'`
Expected: 3 tests PASS

- [ ] **Step 7: Run full workspace checks**

Run: `cargo clippy --workspace --all-targets --all-features` and `cargo fmt --all --check`
Expected: 0 warnings, 0 format issues

- [ ] **Step 8: Commit**

```bash
git add crates/config/src/schema/user.rs crates/config/src/schema/mod.rs crates/config/src/schema/core.rs
git commit -m "feat(config): add UserConfig section for onboarding user profile"
```

---

## Chunk 2: Frontend Foundation — Schema, Types, and Hooks

### Task 2: Create ConversationNode schema and types

**Files:**
- Create: `desktop-ui/src/features/setup/schema.ts`

This is the declarative schema that defines the entire onboarding flow. Each node maps to a prompt sentence with an `{input}` placeholder.

- [ ] **Step 1: Create `schema.ts`**

```typescript
import { ipc } from "@shared/hooks/useIpc";

// ── Types ─────────────────────────────────────────────────────────

export type InputType = "text" | "select" | "masked" | "confirm" | "tags" | "complex";

export type NodeValue = string | boolean | string[];

export type TranscriptValues = Record<string, NodeValue>;

export interface ConversationNode {
  id: string;
  prompt: string; // Sentence template with {input} placeholder
  inputType: InputType;
  default?: NodeValue;
  options?: { label: string; value: string }[];
  validate?: (value: string) => string | null;
  condition?: (values: TranscriptValues) => boolean;
  save?: (value: NodeValue, values: TranscriptValues) => Promise<void>;
  load?: () => Promise<NodeValue | null>;
}

// ── Shared Constants ──────────────────────────────────────────────

export const AREA_COLORS = [
  "#f97316", "#ef4444", "#8b5cf6", "#3b82f6",
  "#06b6d4", "#10b981", "#eab308", "#ec4899",
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
      const data = await ipc<{ name?: string }>("config_get_section", { section: "user" }).catch(() => null);
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
      const data = await ipc<{ defaults?: { provider?: string } }>("config_get_section", { section: "agents" }).catch(() => null);
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
      const providers = await ipc<Record<string, { apiKey?: string }>>("config_get_section", { section: "providers" }).catch(() => null);
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
            name: name.trim(),
            color: AREA_COLORS[(existingNames.size + i) % AREA_COLORS.length],
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
    id: "complete",
    prompt: "Great, we're all set. Let's get started!",
    inputType: "text", // unused — no input on complete node
    save: async () => {
      await ipc("config_mark_setup_completed");
    },
  },
];
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `cd desktop-ui && bun run build 2>&1 | head -20`
Expected: No type errors from schema.ts (other errors from missing components are OK at this stage)

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/setup/schema.ts
git commit -m "feat(setup): add conversational wizard schema and types"
```

### Task 3: Create useTypewriter hook

**Files:**
- Create: `desktop-ui/src/features/setup/hooks/useTypewriter.ts`

This hook drives the character-by-character typewriter animation for prompt text.

- [ ] **Step 1: Create `useTypewriter.ts`**

```typescript
import { useCallback, useEffect, useRef, useState } from "react";

interface UseTypewriterOptions {
  text: string;
  speed?: number; // ms per character, default 30
  onComplete?: () => void;
}

export function useTypewriter({ text, speed = 30, onComplete }: UseTypewriterOptions) {
  const [displayed, setDisplayed] = useState("");
  const [isAnimating, setIsAnimating] = useState(false);
  const timerRef = useRef<ReturnType<typeof setInterval>>();
  const indexRef = useRef(0);
  const completeRef = useRef(onComplete);
  completeRef.current = onComplete;

  const skip = useCallback(() => {
    if (timerRef.current) clearInterval(timerRef.current);
    setDisplayed(text);
    setIsAnimating(false);
    completeRef.current?.();
  }, [text]);

  useEffect(() => {
    if (!text) {
      setDisplayed("");
      setIsAnimating(false);
      return;
    }

    indexRef.current = 0;
    setDisplayed("");
    setIsAnimating(true);

    timerRef.current = setInterval(() => {
      indexRef.current += 1;
      const next = text.slice(0, indexRef.current);
      setDisplayed(next);

      if (indexRef.current >= text.length) {
        clearInterval(timerRef.current);
        setIsAnimating(false);
        completeRef.current?.();
      }
    }, speed);

    return () => {
      if (timerRef.current) clearInterval(timerRef.current);
    };
  }, [text, speed]);

  return { displayed, isAnimating, skip };
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/setup/hooks/useTypewriter.ts
git commit -m "feat(setup): add useTypewriter hook for conversational animation"
```

### Task 4: Create useConversationRunner hook

**Files:**
- Create: `desktop-ui/src/features/setup/hooks/useConversationRunner.ts`

Core state management for the conversational wizard — manages transcript, active index, save/load, edit-in-place.

- [ ] **Step 1: Create `useConversationRunner.ts`**

```typescript
import { useCallback, useEffect, useRef, useState } from "react";
import type { ConversationNode, NodeValue, TranscriptValues } from "../schema";
import { CONVERSATION_SCHEMA } from "../schema";

export interface TranscriptEntry {
  node: ConversationNode;
  value: NodeValue;
  status: "completed" | "editing";
}

interface RunnerState {
  transcript: Record<string, TranscriptEntry>;
  activeIndex: number;
  isAnimating: boolean;
  error: string | null;
  isSaving: boolean;
  isLoading: boolean;
}

export function useConversationRunner() {
  const [state, setState] = useState<RunnerState>({
    transcript: {},
    activeIndex: 0,
    isAnimating: true, // Start animating the first prompt
    error: null,
    isSaving: false,
    isLoading: true,
  });

  const schema = CONVERSATION_SCHEMA;
  const stateRef = useRef(state);
  stateRef.current = state;

  // ── Derived values ──────────────────────────────────────────

  const transcriptValues: TranscriptValues = {};
  for (const [id, entry] of Object.entries(state.transcript)) {
    if (entry.status === "completed") {
      transcriptValues[id] = entry.value;
    }
  }

  const activeNode = schema[state.activeIndex] ?? null;
  const completedCount = Object.values(state.transcript).filter(
    (e) => e.status === "completed",
  ).length;
  const totalNodes = schema.filter((n) => n.id !== "complete" && n.id !== "finance_setup").length;
  const progress = totalNodes > 0 ? completedCount / totalNodes : 0;

  // ── Resume: load existing values ────────────────────────────

  useEffect(() => {
    let cancelled = false;

    async function loadExisting() {
      const loaded: Record<string, TranscriptEntry> = {};
      let firstEmpty = 0;

      for (let i = 0; i < schema.length; i++) {
        const node = schema[i];

        // Skip nodes with conditions that depend on not-yet-loaded values
        if (node.condition && !node.condition({})) continue;
        // Skip nodes without loaders
        if (!node.load) {
          firstEmpty = i;
          break;
        }

        try {
          const value = await node.load();
          if (value !== null && value !== undefined && value !== "") {
            loaded[node.id] = { node, value, status: "completed" };
          } else {
            firstEmpty = i;
            break;
          }
        } catch {
          firstEmpty = i;
          break;
        }
      }

      if (cancelled) return;
      setState((prev) => ({
        ...prev,
        transcript: loaded,
        activeIndex: firstEmpty,
        isLoading: false,
        isAnimating: true,
      }));
    }

    loadExisting();
    return () => { cancelled = true; };
  // eslint-disable-next-line react-hooks/exhaustive-deps -- schema is a module constant, runs once on mount
  }, []);

  // ── Find next valid index (skipping conditions) ─────────────

  const findNextIndex = useCallback(
    (fromIndex: number, values: TranscriptValues): number => {
      for (let i = fromIndex + 1; i < schema.length; i++) {
        const node = schema[i];
        if (!node.condition || node.condition(values)) return i;
      }
      return schema.length; // past the end — all done
    },
    [schema],
  );

  // ── Submit current node ─────────────────────────────────────

  const submit = useCallback(
    async (value: NodeValue) => {
      const { activeIndex, transcript } = stateRef.current;
      const node = schema[activeIndex];
      if (!node) return;

      // Validate
      if (node.validate && typeof value === "string") {
        const err = node.validate(value);
        if (err) {
          setState((prev) => ({ ...prev, error: err }));
          return;
        }
      }

      setState((prev) => ({ ...prev, isSaving: true, error: null }));

      // Build current values including this one
      const currentValues: TranscriptValues = {};
      for (const [id, entry] of Object.entries(transcript)) {
        if (entry.status === "completed") currentValues[id] = entry.value;
      }
      currentValues[node.id] = value;

      // Save
      try {
        if (node.save) {
          await node.save(value, currentValues);
        }
      } catch (e) {
        setState((prev) => ({
          ...prev,
          isSaving: false,
          error: e instanceof Error ? e.message : "Failed to save",
        }));
        return;
      }

      const nextIndex = findNextIndex(activeIndex, currentValues);
      setState((prev) => ({
        ...prev,
        transcript: {
          ...prev.transcript,
          [node.id]: { node, value, status: "completed" },
        },
        activeIndex: nextIndex,
        isSaving: false,
        isAnimating: true,
        error: null,
      }));
    },
    [schema, findNextIndex],
  );

  // ── Re-submit after edit ────────────────────────────────────

  const resubmit = useCallback(
    async (nodeId: string, value: NodeValue) => {
      const entry = stateRef.current.transcript[nodeId];
      if (!entry) return;

      setState((prev) => ({ ...prev, isSaving: true, error: null }));

      const currentValues: TranscriptValues = {};
      for (const [id, e] of Object.entries(stateRef.current.transcript)) {
        if (e.status === "completed") currentValues[id] = e.value;
      }
      currentValues[nodeId] = value;

      try {
        if (entry.node.save) {
          await entry.node.save(value, currentValues);
        }
      } catch (e) {
        setState((prev) => ({
          ...prev,
          isSaving: false,
          error: e instanceof Error ? e.message : "Failed to save",
        }));
        return;
      }

      setState((prev) => ({
        ...prev,
        transcript: {
          ...prev.transcript,
          [nodeId]: { ...prev.transcript[nodeId], value, status: "completed" },
        },
        isSaving: false,
        error: null,
      }));
    },
    [],
  );

  // ── Edit a completed node ───────────────────────────────────

  const startEdit = useCallback((nodeId: string) => {
    setState((prev) => ({
      ...prev,
      transcript: {
        ...prev.transcript,
        [nodeId]: { ...prev.transcript[nodeId], status: "editing" },
      },
      error: null,
    }));
  }, []);

  const cancelEdit = useCallback((nodeId: string) => {
    setState((prev) => ({
      ...prev,
      transcript: {
        ...prev.transcript,
        [nodeId]: { ...prev.transcript[nodeId], status: "completed" },
      },
    }));
  }, []);

  // ── Animation complete ──────────────────────────────────────

  const setAnimationComplete = useCallback(() => {
    setState((prev) => ({ ...prev, isAnimating: false }));
  }, []);

  // ── Finance panel complete ──────────────────────────────────

  const completeFinancePanel = useCallback(() => {
    const currentValues: TranscriptValues = {};
    for (const [id, entry] of Object.entries(stateRef.current.transcript)) {
      if (entry.status === "completed") currentValues[id] = entry.value;
    }
    const financeNode = schema.find((n) => n.id === "finance_setup");
    if (financeNode) {
      const nextIndex = findNextIndex(stateRef.current.activeIndex, currentValues);
      setState((prev) => ({
        ...prev,
        transcript: {
          ...prev.transcript,
          finance_setup: { node: financeNode, value: true, status: "completed" },
        },
        activeIndex: nextIndex,
        isAnimating: true,
      }));
    }
  }, [schema, findNextIndex]);

  return {
    // State
    transcript: state.transcript,
    activeNode,
    activeIndex: state.activeIndex,
    isAnimating: state.isAnimating,
    error: state.error,
    isSaving: state.isSaving,
    isLoading: state.isLoading,
    progress,
    schema,
    transcriptValues,
    // Actions
    submit,
    resubmit,
    startEdit,
    cancelEdit,
    setAnimationComplete,
    completeFinancePanel,
  };
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `cd desktop-ui && bun run build 2>&1 | head -20`
Expected: No type errors from the new hook

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/setup/hooks/useConversationRunner.ts
git commit -m "feat(setup): add useConversationRunner state management hook"
```

---

## Chunk 3: Inline Input Components

### Task 5: Create InlineInput (text)

**Files:**
- Create: `desktop-ui/src/features/setup/components/InlineInput.tsx`

Renders an underlined text input inline within a sentence. Auto-focuses. Submits on Enter.

- [ ] **Step 1: Create `InlineInput.tsx`**

```tsx
import { useEffect, useRef, useState } from "react";

interface InlineInputProps {
  defaultValue?: string;
  placeholder?: string;
  onSubmit: (value: string) => void;
  disabled?: boolean;
  autoFocus?: boolean;
}

export function InlineInput({
  defaultValue = "",
  placeholder = "...",
  onSubmit,
  disabled,
  autoFocus = true,
}: InlineInputProps) {
  const [value, setValue] = useState(defaultValue);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (autoFocus) inputRef.current?.focus();
  }, [autoFocus]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !disabled) {
      e.preventDefault();
      onSubmit(value);
    }
  };

  return (
    <input
      ref={inputRef}
      type="text"
      value={value}
      onChange={(e) => setValue(e.target.value)}
      onKeyDown={handleKeyDown}
      placeholder={placeholder}
      disabled={disabled}
      className="inline-block border-b-2 border-accent bg-transparent text-accent font-semibold outline-none min-w-[120px] placeholder:text-muted/50 disabled:opacity-50 transition-colors"
      style={{ width: `${Math.max(value.length, placeholder.length) + 2}ch` }}
    />
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/setup/components/InlineInput.tsx
git commit -m "feat(setup): add InlineInput component for conversational text input"
```

### Task 6: Create InlineSelect (dropdown)

**Files:**
- Create: `desktop-ui/src/features/setup/components/InlineSelect.tsx`

Renders as underlined text showing the selected value. On click, a portal-based dropdown opens below.

- [ ] **Step 1: Create `InlineSelect.tsx`**

```tsx
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

interface InlineSelectProps {
  options: { label: string; value: string }[];
  defaultValue?: string;
  onSubmit: (value: string) => void;
  disabled?: boolean;
  autoFocus?: boolean;
}

export function InlineSelect({
  options,
  defaultValue,
  onSubmit,
  disabled,
  autoFocus = true,
}: InlineSelectProps) {
  const initial = defaultValue || options[0]?.value || "";
  const [value, setValue] = useState(initial);
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const [dropdownPos, setDropdownPos] = useState({ top: 0, left: 0 });

  const selectedLabel = options.find((o) => o.value === value)?.label ?? value;

  useEffect(() => {
    if (autoFocus) triggerRef.current?.focus();
  }, [autoFocus]);

  const openDropdown = () => {
    if (disabled) return;
    const rect = triggerRef.current?.getBoundingClientRect();
    if (rect) {
      setDropdownPos({ top: rect.bottom + 4, left: rect.left });
    }
    setOpen(true);
  };

  const select = (val: string) => {
    setValue(val);
    setOpen(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !open && !disabled) {
      e.preventDefault();
      onSubmit(value);
    }
  };

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        onClick={openDropdown}
        onKeyDown={handleKeyDown}
        disabled={disabled}
        className="inline-block border-b-2 border-accent bg-transparent text-accent font-semibold outline-none cursor-pointer hover:border-accent/70 transition-colors disabled:opacity-50"
      >
        {selectedLabel} <span className="text-muted/50 text-xs">&#9662;</span>
      </button>

      {open &&
        createPortal(
          <>
            {/* Backdrop */}
            <div className="fixed inset-0 z-50" onClick={() => setOpen(false)} />
            {/* Dropdown */}
            <div
              className="fixed z-50 glass-panel border border-border rounded-lg py-1 shadow-lg min-w-[160px]"
              style={{ top: dropdownPos.top, left: dropdownPos.left }}
            >
              {options.map((opt) => (
                <button
                  key={opt.value}
                  type="button"
                  onClick={() => select(opt.value)}
                  className={`block w-full text-left px-3 py-1.5 text-[13px] transition-colors ${
                    opt.value === value
                      ? "text-accent bg-accent/10"
                      : "text-foreground hover:bg-white/[0.06]"
                  }`}
                >
                  {opt.label}
                </button>
              ))}
            </div>
          </>,
          document.body,
        )}
    </>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/setup/components/InlineSelect.tsx
git commit -m "feat(setup): add InlineSelect component with portal dropdown"
```

### Task 7: Create InlineMasked (API key input)

**Files:**
- Create: `desktop-ui/src/features/setup/components/InlineMasked.tsx`

Inline masked input — shows dots with eye toggle. Reuses patterns from existing `SecretInput` at `desktop-ui/src/shared/ui/SecretInput.tsx`.

- [ ] **Step 1: Create `InlineMasked.tsx`**

```tsx
import { Eye, EyeOff } from "lucide-react";
import { useEffect, useRef, useState } from "react";

interface InlineMaskedProps {
  defaultValue?: string;
  onSubmit: (value: string) => void;
  disabled?: boolean;
  autoFocus?: boolean;
}

export function InlineMasked({
  defaultValue = "",
  onSubmit,
  disabled,
  autoFocus = true,
}: InlineMaskedProps) {
  const [value, setValue] = useState(defaultValue);
  const [show, setShow] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (autoFocus) inputRef.current?.focus();
  }, [autoFocus]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !disabled) {
      e.preventDefault();
      onSubmit(value);
    }
  };

  return (
    <span className="inline-flex items-center gap-1">
      <input
        ref={inputRef}
        type={show ? "text" : "password"}
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="paste your key"
        disabled={disabled}
        className="inline-block border-b-2 border-accent bg-transparent text-accent font-semibold outline-none min-w-[200px] placeholder:text-muted/50 disabled:opacity-50 transition-colors"
      />
      <button
        type="button"
        onClick={() => setShow(!show)}
        className="text-muted hover:text-secondary transition-colors"
      >
        {show ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
      </button>
    </span>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/setup/components/InlineMasked.tsx
git commit -m "feat(setup): add InlineMasked component for API key input"
```

### Task 8: Create InlineTags (areas input)

**Files:**
- Create: `desktop-ui/src/features/setup/components/InlineTags.tsx`

Tag/pill input for entering area names. Comma or Enter adds a tag. Empty Enter confirms and submits all tags.

- [ ] **Step 1: Create `InlineTags.tsx`**

```tsx
import { X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { AREA_COLORS } from "../schema";

interface InlineTagsProps {
  defaultValue?: string[];
  onSubmit: (tags: string[]) => void;
  disabled?: boolean;
  autoFocus?: boolean;
}

export function InlineTags({
  defaultValue = [],
  onSubmit,
  disabled,
  autoFocus = true,
}: InlineTagsProps) {
  const [tags, setTags] = useState<string[]>(defaultValue);
  const [input, setInput] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (autoFocus) inputRef.current?.focus();
  }, [autoFocus]);

  const addTag = (name: string) => {
    const trimmed = name.trim();
    if (trimmed && !tags.includes(trimmed)) {
      setTags((prev) => [...prev, trimmed]);
    }
    setInput("");
  };

  const removeTag = (index: number) => {
    setTags((prev) => prev.filter((_, i) => i !== index));
    inputRef.current?.focus();
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (disabled) return;

    if (e.key === "Enter") {
      e.preventDefault();
      if (input.trim()) {
        addTag(input);
      } else if (tags.length > 0) {
        // Empty enter = confirm all tags
        onSubmit(tags);
      }
    } else if (e.key === "," || e.key === "Tab") {
      e.preventDefault();
      if (input.trim()) addTag(input);
    } else if (e.key === "Backspace" && !input && tags.length > 0) {
      removeTag(tags.length - 1);
    }
  };

  return (
    <span className="inline-flex flex-wrap items-center gap-1.5">
      {tags.map((tag, i) => (
        <span
          key={tag}
          className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[13px] font-medium text-white"
          style={{ backgroundColor: AREA_COLORS[i % AREA_COLORS.length] }}
        >
          {tag}
          {!disabled && (
            <button
              type="button"
              onClick={() => removeTag(i)}
              className="hover:opacity-70 transition-opacity"
            >
              <X className="w-3 h-3" />
            </button>
          )}
        </span>
      ))}
      <input
        ref={inputRef}
        type="text"
        value={input}
        onChange={(e) => setInput(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={tags.length === 0 ? "type and press Enter" : "add more or press Enter to confirm"}
        disabled={disabled}
        className="inline-block border-b-2 border-accent bg-transparent text-accent font-semibold outline-none min-w-[120px] placeholder:text-muted/50 text-[13px] disabled:opacity-50 transition-colors"
      />
    </span>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/setup/components/InlineTags.tsx
git commit -m "feat(setup): add InlineTags component for area selection"
```

### Task 9: Create TypewriterText component

**Files:**
- Create: `desktop-ui/src/features/setup/components/TypewriterText.tsx`

Wraps the `useTypewriter` hook into a component that renders the prompt text with typewriter animation, splitting at `{input}` to inject the input component.

- [ ] **Step 1: Create `TypewriterText.tsx`**

```tsx
import type { ReactNode } from "react";
import { useTypewriter } from "../hooks/useTypewriter";

interface TypewriterTextProps {
  text: string; // Full prompt with {input} placeholder
  input?: ReactNode; // The inline input component
  onComplete?: () => void;
  showInput?: boolean; // Show input after typewriter finishes
}

export function TypewriterText({ text, input, onComplete, showInput = false }: TypewriterTextProps) {
  const parts = text.split("{input}");
  const before = parts[0] ?? "";
  const after = parts[1] ?? "";
  const hasInput = text.includes("{input}");

  // Animate only the "before" part — stop at the blank
  const { displayed, isAnimating, skip } = useTypewriter({
    text: before,
    onComplete,
  });

  const handleClick = () => {
    if (isAnimating) skip();
  };

  // During animation: typewriter the text before the blank
  if (isAnimating) {
    return (
      <span onClick={handleClick} className="cursor-pointer">
        {displayed}
      </span>
    );
  }

  // After animation: show before + (blank or input) + after
  if (!showInput && hasInput) {
    // Show a visual blank placeholder (underlined space)
    return (
      <span onClick={handleClick} className="cursor-pointer">
        {before}
        <span className="inline-block border-b-2 border-accent/40 min-w-[80px]">&nbsp;</span>
        {after}
      </span>
    );
  }

  // Show: before + live input + after
  return (
    <span>
      {before}
      {hasInput ? input : null}
      {after}
    </span>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/setup/components/TypewriterText.tsx
git commit -m "feat(setup): add TypewriterText component for animated prompts"
```

---

## Chunk 4: ConversationRunner and FinancePanel

### Task 10: Create FinancePanel

**Files:**
- Create: `desktop-ui/src/features/setup/components/FinancePanel.tsx`

Slide-down panel wrapping existing finance sub-forms. Reimplements `FinanceStep`'s sub-step navigation using props instead of `useOutletContext`.

- [ ] **Step 1: Create `FinancePanel.tsx`**

```tsx
import { useCallback, useMemo, useRef, useState } from "react";
import { AccountsForm } from "./finance/AccountsForm";
import { FinanceBasicsForm } from "./finance/FinanceBasicsForm";
import { FireForm } from "./finance/FireForm";
import { GoalsForm } from "./finance/GoalsForm";
import { IncomeForm } from "./finance/IncomeForm";
import { InvestmentsForm } from "./finance/InvestmentsForm";
import { LiabilitiesForm } from "./finance/LiabilitiesForm";

const SUB_STEPS = [
  "basics", "accounts", "budgeting", "fire", "investments", "liabilities", "goals",
] as const;
const SUB_STEP_LABELS: Record<(typeof SUB_STEPS)[number], string> = {
  basics: "Basics",
  accounts: "Accounts",
  budgeting: "Budgeting",
  fire: "FIRE",
  investments: "Investments",
  liabilities: "Liabilities",
  goals: "Goals",
};

interface FinancePanelProps {
  onComplete: () => void;
}

export function FinancePanel({ onComplete }: FinancePanelProps) {
  const [subStep, setSubStep] = useState(0);
  const [saving, setSaving] = useState(false);
  const subSaveMap = useRef<Map<number, () => Promise<void>>>(new Map());

  const makeRegisterSave = useCallback(
    (index: number) => (fn: () => Promise<void>) => {
      subSaveMap.current.set(index, fn);
    },
    [],
  );

  const registerSaves = useMemo(
    () => SUB_STEPS.map((_, i) => makeRegisterSave(i)),
    [makeRegisterSave],
  );

  const markDirty = useCallback(() => {}, []); // No-op — panel doesn't need dirty tracking

  const handleNext = async () => {
    setSaving(true);
    try {
      const save = subSaveMap.current.get(subStep);
      await save?.();
    } finally {
      setSaving(false);
    }

    if (subStep < SUB_STEPS.length - 1) {
      setSubStep((s) => s + 1);
    } else {
      onComplete();
    }
  };

  const handleBack = () => {
    if (subStep > 0) setSubStep((s) => s - 1);
  };

  return (
    <div className="mt-4 bg-surface-low rounded-xl border border-border p-6 animate-in slide-in-from-top-2 duration-300">
      {/* Mini progress */}
      <div className="flex items-center gap-1 mb-5">
        {SUB_STEPS.map((step, i) => (
          <button
            key={step}
            type="button"
            onClick={() => setSubStep(i)}
            className={`text-[10px] px-2 py-0.5 rounded-full transition-colors ${
              i === subStep
                ? "bg-brand text-white"
                : i < subStep
                  ? "bg-brand/20 text-brand"
                  : "bg-white/[0.06] text-dim"
            }`}
          >
            {SUB_STEP_LABELS[step]}
          </button>
        ))}
      </div>

      {/* Sub-forms — all rendered, hidden when inactive */}
      <div className={subStep !== 0 ? "hidden" : undefined}>
        <FinanceBasicsForm registerSave={registerSaves[0]} onDirty={markDirty} />
      </div>
      <div className={subStep !== 1 ? "hidden" : undefined}>
        <AccountsForm registerSave={registerSaves[1]} onDirty={markDirty} />
      </div>
      <div className={subStep !== 2 ? "hidden" : undefined}>
        <IncomeForm registerSave={registerSaves[2]} onDirty={markDirty} />
      </div>
      <div className={subStep !== 3 ? "hidden" : undefined}>
        <FireForm registerSave={registerSaves[3]} onDirty={markDirty} />
      </div>
      <div className={subStep !== 4 ? "hidden" : undefined}>
        <InvestmentsForm registerSave={registerSaves[4]} onDirty={markDirty} />
      </div>
      <div className={subStep !== 5 ? "hidden" : undefined}>
        <LiabilitiesForm registerSave={registerSaves[5]} onDirty={markDirty} />
      </div>
      <div className={subStep !== 6 ? "hidden" : undefined}>
        <GoalsForm registerSave={registerSaves[6]} onDirty={markDirty} />
      </div>

      {/* Navigation */}
      <div className="flex items-center justify-between mt-6 pt-4 border-t border-border">
        <div className="flex gap-2">
          {subStep > 0 && (
            <button
              type="button"
              onClick={handleBack}
              className="px-3 py-1.5 text-[12px] text-muted hover:text-secondary transition-colors"
            >
              Back
            </button>
          )}
          <button
            type="button"
            onClick={onComplete}
            className="px-3 py-1.5 text-[12px] text-muted hover:text-secondary transition-colors"
          >
            Skip
          </button>
        </div>
        <button
          type="button"
          onClick={handleNext}
          disabled={saving}
          className="px-4 py-1.5 text-[12px] font-medium text-white bg-brand hover:bg-brand-hover rounded-lg transition-colors disabled:opacity-50"
        >
          {saving ? "Saving..." : subStep === SUB_STEPS.length - 1 ? "Done" : "Next"}
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/setup/components/FinancePanel.tsx
git commit -m "feat(setup): add FinancePanel component with sub-step navigation"
```

### Task 11: Create ConversationRunner

**Files:**
- Create: `desktop-ui/src/features/setup/components/ConversationRunner.tsx`

The main engine component that renders the conversation flow, delegates to inline inputs, handles animations, and manages the edit-in-place behavior.

- [ ] **Step 1: Create `ConversationRunner.tsx`**

```tsx
import { useCallback, useRef } from "react";
import { useNavigate } from "react-router";
import { useConversationRunner } from "../hooks/useConversationRunner";
import type { ConversationNode, NodeValue } from "../schema";
import { FinancePanel } from "./FinancePanel";
import { InlineInput } from "./InlineInput";
import { InlineMasked } from "./InlineMasked";
import { InlineSelect } from "./InlineSelect";
import { InlineTags } from "./InlineTags";
import { TypewriterText } from "./TypewriterText";

// ── Completed node (locked text, click to edit) ──────────────

function CompletedNode({
  node,
  value,
  isEditing,
  onClickEdit,
  onResubmit,
}: {
  node: ConversationNode;
  value: NodeValue;
  isEditing: boolean;
  onClickEdit: () => void;
  onResubmit: (value: NodeValue) => void;
}) {
  const parts = node.prompt.split("{input}");
  const before = parts[0] ?? "";
  const after = parts[1] ?? "";

  if (isEditing) {
    return (
      <div className="text-foreground text-base leading-relaxed">
        <span>{before}</span>
        {renderInput(node, value, onResubmit, true)}
        <span>{after}</span>
      </div>
    );
  }

  // Display completed value
  const displayValue = formatValue(node, value);
  return (
    <div
      className="text-foreground text-base leading-relaxed cursor-pointer group"
      onClick={onClickEdit}
    >
      <span>{before}</span>
      <span className="font-semibold group-hover:text-accent transition-colors">
        {displayValue}
      </span>
      <span>{after}</span>
    </div>
  );
}

function formatValue(node: ConversationNode, value: NodeValue): string {
  if (node.inputType === "select" || node.inputType === "confirm") {
    if (node.inputType === "confirm") return value ? "Yes" : "No";
    const opt = node.options?.find((o) => o.value === value);
    return opt?.label ?? String(value);
  }
  if (node.inputType === "masked" && typeof value === "string") {
    return value.length > 8 ? `${"•".repeat(8)}${value.slice(-4)}` : "•".repeat(value.length);
  }
  if (Array.isArray(value)) return value.join(", ");
  return String(value);
}

// ── Render input by type ─────────────────────────────────────

function renderInput(
  node: ConversationNode,
  defaultValue: NodeValue | undefined,
  onSubmit: (value: NodeValue) => void,
  autoFocus = true,
) {
  switch (node.inputType) {
    case "text":
      return (
        <InlineInput
          defaultValue={typeof defaultValue === "string" ? defaultValue : ""}
          onSubmit={onSubmit}
          autoFocus={autoFocus}
        />
      );
    case "select":
      return (
        <InlineSelect
          options={node.options ?? []}
          defaultValue={typeof defaultValue === "string" ? defaultValue : (node.default as string)}
          onSubmit={onSubmit}
          autoFocus={autoFocus}
        />
      );
    case "masked":
      return (
        <InlineMasked
          defaultValue={typeof defaultValue === "string" ? defaultValue : ""}
          onSubmit={onSubmit}
          autoFocus={autoFocus}
        />
      );
    case "confirm":
      return (
        <InlineSelect
          options={[
            { label: "Yes", value: "true" },
            { label: "No", value: "false" },
          ]}
          defaultValue={defaultValue === true || defaultValue === "true" || node.default === true ? "true" : "false"}
          onSubmit={(v) => onSubmit(v === "true")}
          autoFocus={autoFocus}
        />
      );
    case "tags":
      return (
        <InlineTags
          defaultValue={Array.isArray(defaultValue) ? defaultValue : (node.default as string[]) ?? []}
          onSubmit={onSubmit}
          autoFocus={autoFocus}
        />
      );
    default:
      return null;
  }
}

// ── Main ConversationRunner ──────────────────────────────────

export function ConversationRunner() {
  const navigate = useNavigate();
  const containerRef = useRef<HTMLDivElement>(null);
  const {
    transcript,
    activeNode,
    activeIndex,
    isAnimating,
    error,
    isSaving,
    isLoading,
    progress,
    schema,
    submit,
    resubmit,
    startEdit,
    cancelEdit,
    setAnimationComplete,
    completeFinancePanel,
  } = useConversationRunner();

  const isAnyEditing = Object.values(transcript).some((e) => e.status === "editing");

  const handleSubmit = useCallback(
    async (value: NodeValue) => {
      await submit(value);
      // Scroll to bottom after new node appears
      requestAnimationFrame(() => {
        containerRef.current?.scrollTo({ top: containerRef.current.scrollHeight, behavior: "smooth" });
      });
    },
    [submit],
  );

  const handleComplete = useCallback(async () => {
    // Must call config_mark_setup_completed before navigating,
    // otherwise app_info().setupCompleted is still false → redirect loop
    const completeNode = schema.find((n) => n.id === "complete");
    if (completeNode?.save) await completeNode.save(true, {});
    navigate("/");
  }, [navigate, schema]);

  if (isLoading) {
    return (
      <div className="fixed inset-0 flex items-center justify-center bg-surface-base">
        <div className="text-muted text-sm">Loading...</div>
      </div>
    );
  }

  const isComplete = activeNode?.id === "complete" || activeIndex >= schema.length;

  return (
    <div className="fixed inset-0 flex flex-col bg-surface-base">
      {/* Progress bar */}
      <div className="h-1 bg-border">
        <div
          className="h-full bg-accent transition-all duration-500 ease-out"
          style={{ width: `${progress * 100}%` }}
        />
      </div>

      {/* Conversation area */}
      <div
        ref={containerRef}
        className="flex-1 overflow-y-auto flex justify-center"
      >
        <div className="w-full max-w-[640px] px-6 py-12 space-y-4">
          {/* Completed & editing nodes */}
          {schema.slice(0, activeIndex).map((node) => {
            const entry = transcript[node.id];
            if (!entry) return null; // Skipped by condition
            if (node.inputType === "complex") return null; // Finance panel handled separately

            return (
              <div
                key={node.id}
                className={`transition-opacity duration-200 ${
                  isAnyEditing && entry.status !== "editing" ? "opacity-40" : ""
                }`}
              >
                <CompletedNode
                  node={node}
                  value={entry.value}
                  isEditing={entry.status === "editing"}
                  onClickEdit={() => !isAnyEditing && startEdit(node.id)}
                  onResubmit={(v) => resubmit(node.id, v)}
                />
              </div>
            );
          })}

          {/* Active node */}
          {activeNode && !isComplete && activeNode.inputType !== "complex" && (
            <div className="text-foreground text-base leading-relaxed">
              <TypewriterText
                text={activeNode.prompt}
                onComplete={setAnimationComplete}
                showInput={!isAnimating}
                input={renderInput(activeNode, activeNode.default, handleSubmit)}
              />
            </div>
          )}

          {/* Finance panel (complex node) */}
          {activeNode?.id === "finance_setup" && (
            <FinancePanel onComplete={completeFinancePanel} />
          )}

          {/* Error message */}
          {error && (
            <p className="text-[12px] text-destructive animate-in fade-in duration-200">
              {error}
            </p>
          )}

          {/* Saving indicator */}
          {isSaving && (
            <p className="text-[12px] text-muted animate-in fade-in duration-200">
              Saving...
            </p>
          )}

          {/* Complete state */}
          {isComplete && (
            <div className="space-y-6">
              <TypewriterText
                text="Great, we're all set. Let's get started!"
                onComplete={() => {}}
              />
              <button
                type="button"
                onClick={handleComplete}
                className="px-6 py-2.5 text-[14px] font-medium text-white bg-brand hover:bg-brand-hover rounded-xl transition-colors"
              >
                Launch Klynt
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `cd desktop-ui && npx tsc --noEmit 2>&1 | head -30`
Expected: No type errors (or only pre-existing unrelated ones)

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/setup/components/ConversationRunner.tsx
git commit -m "feat(setup): add ConversationRunner engine component"
```

---

## Chunk 5: Wiring — Router, Index, Cleanup

**IMPORTANT: Tasks 12-14 must be done together in a single commit. The index.ts update, router update, and file deletions are interdependent — an intermediate state where only some are done will break the build.**

### Task 12: Update index.ts exports

**Files:**
- Modify: `desktop-ui/src/features/setup/index.ts`

Remove old page exports, add new component exports.

- [ ] **Step 1: Rewrite `index.ts`**

Replace the entire contents of `desktop-ui/src/features/setup/index.ts`:

```typescript
// Conversational setup wizard
export { ConversationRunner } from "./components/ConversationRunner";

// Finance sub-components (kept for reuse)
export { AccountsForm } from "./components/finance/AccountsForm";
export { FinanceBasicsForm } from "./components/finance/FinanceBasicsForm";
export { FireForm } from "./components/finance/FireForm";
export { GoalsForm } from "./components/finance/GoalsForm";
export { IncomeForm } from "./components/finance/IncomeForm";
export { InvestmentsForm } from "./components/finance/InvestmentsForm";
export { LiabilitiesForm } from "./components/finance/LiabilitiesForm";
```

- [ ] **Step 2: Do NOT commit yet — continue to Task 13**

### Task 13: Update router.tsx

**Files:**
- Modify: `desktop-ui/src/app/router.tsx`

Replace multi-step setup routes with a single route rendering `ConversationRunner`. Remove old lazy imports.

- [ ] **Step 1: Update router setup section**

In `desktop-ui/src/app/router.tsx`:

Replace the setup wizard lazy imports (lines 126-148) with:

```typescript
// ── Setup Wizard ─────────────────────────────────────────────────
const ConversationRunner = lazy(() =>
  import("../features/setup").then((m) => ({ default: m.ConversationRunner })),
);
```

Replace the setup routes block (lines 299-313):

```typescript
  { path: "/setup", element: <ConversationRunner /> },
  { path: "/setup/*", element: <ConversationRunner /> },
```

- [ ] **Step 2: Update SetupRedirect**

In `SetupRedirect` function (around line 155), change the redirect target from `/setup/welcome` to `/setup`:

```typescript
.then((info) => setTarget(info.setupCompleted ? "/" : "/setup"))
```

- [ ] **Step 3: Verify build compiles**

Run: `cd desktop-ui && bun run build 2>&1 | tail -10`
Expected: Build succeeds

- [ ] **Step 4: Do NOT commit yet — continue to Task 14**

### Task 14: Delete old step pages and unused hooks

**Files:**
- Delete: `desktop-ui/src/features/setup/pages/WelcomeStep.tsx`
- Delete: `desktop-ui/src/features/setup/pages/ProviderStep.tsx`
- Delete: `desktop-ui/src/features/setup/pages/ChannelsStep.tsx`
- Delete: `desktop-ui/src/features/setup/pages/AreasStep.tsx`
- Delete: `desktop-ui/src/features/setup/pages/ProductivityStep.tsx`
- Delete: `desktop-ui/src/features/setup/pages/FinanceStep.tsx`
- Delete: `desktop-ui/src/features/setup/pages/McpStep.tsx`
- Delete: `desktop-ui/src/features/setup/pages/CompleteStep.tsx`
- Delete: `desktop-ui/src/features/setup/hooks/steps.ts`
- Delete: `desktop-ui/src/features/setup/hooks/useSetupNavigation.ts`
- Delete: `desktop-ui/src/features/setup/components/SetupLayout.tsx`
- Delete: `desktop-ui/src/features/setup/components/SetupProgress.tsx`

- [ ] **Step 1: Delete files**

```bash
rm desktop-ui/src/features/setup/pages/WelcomeStep.tsx
rm desktop-ui/src/features/setup/pages/ProviderStep.tsx
rm desktop-ui/src/features/setup/pages/ChannelsStep.tsx
rm desktop-ui/src/features/setup/pages/AreasStep.tsx
rm desktop-ui/src/features/setup/pages/ProductivityStep.tsx
rm desktop-ui/src/features/setup/pages/FinanceStep.tsx
rm desktop-ui/src/features/setup/pages/McpStep.tsx
rm desktop-ui/src/features/setup/pages/CompleteStep.tsx
rm desktop-ui/src/features/setup/hooks/steps.ts
rm desktop-ui/src/features/setup/hooks/useSetupNavigation.ts
rm desktop-ui/src/features/setup/components/SetupLayout.tsx
rm desktop-ui/src/features/setup/components/SetupProgress.tsx
```

- [ ] **Step 2: Remove pages directory if empty**

```bash
rmdir desktop-ui/src/features/setup/pages 2>/dev/null || true
```

- [ ] **Step 3: Verify build still compiles**

Run: `cd desktop-ui && bun run build 2>&1 | tail -10`
Expected: Build succeeds — all old imports removed in previous tasks

- [ ] **Step 4: Run lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: No errors

- [ ] **Step 5: Commit all of Tasks 12-14 together**

```bash
git add desktop-ui/src/features/setup/ desktop-ui/src/app/router.tsx
git commit -m "refactor(setup): replace old wizard with conversational runner

Remove all old step pages, SetupLayout, SetupProgress, step definitions,
and navigation hook. Simplify router to single ConversationRunner route.
Update index.ts exports."
```

---

## Chunk 6: Integration Testing and Polish

### Task 15: Verify the complete flow

- [ ] **Step 1: Run full backend build and tests**

Run: `cargo nextest run --workspace` and `cargo clippy --workspace --all-targets --all-features`
Expected: All tests pass, 0 clippy warnings

- [ ] **Step 2: Run frontend build and lint**

Run: `cd desktop-ui && bun run build && bun run lint:fix`
Expected: Clean build, no lint errors

- [ ] **Step 3: Manual smoke test**

Run: `cargo tauri dev`

Verify:
1. On first run (no config), the app shows the conversational wizard at `/setup`
2. First prompt typewriter-animates: "Hello, I'm Klynt. Your name is ___."
3. Type a name, press Enter — it locks in, next prompt animates
4. Provider dropdown works inline, press Enter to confirm
5. API key masked input works, Eye toggle reveals text
6. Areas tags input: type names, comma/Enter to add pills, Enter on empty to confirm
7. Productivity gate: Yes/No dropdown, Enter to confirm
8. Finance gate: No → skips panel. Yes → panel slides down with sub-forms
9. "Great, we're all set" message + Launch Klynt button
10. Clicking a completed answer re-opens it for editing
11. Close app mid-setup, reopen — resumes from last unfilled node

- [ ] **Step 4: Run frontend tests**

Run: `cd desktop-ui && bun run test`
Expected: All existing tests pass (no regressions)

- [ ] **Step 5: Final commit if any polish changes were needed**

```bash
git add -A
git commit -m "fix(setup): polish conversational wizard integration"
```
