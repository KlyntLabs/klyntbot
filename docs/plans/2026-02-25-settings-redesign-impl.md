# Settings Page Redesign Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rewrite the Settings page from a 2025-line monolith with one-card-per-field layout into a dense VS Code/Linear-style interface with inline rows, collapsible sub-sections, smart selects, and 8 consolidated nav groups.

**Architecture:** Extract reusable setting row components into `src/app/components/settings/`. Rewrite `Settings.tsx` to use these components with the new 8-section grouping. No backend changes — same PATCH API, same config paths. Use `sonner` toast for save feedback (already installed). Keep existing `useApi`, `apiFetch`, `patchSection`, `debouncedPatch` patterns.

**Tech Stack:** React 19, TypeScript, Tailwind v4, lucide-react icons, sonner (toast), existing `useApi`/`apiFetch` hooks.

---

### Task 1: Create Shared Setting Components

**Files:**
- Create: `crates/dashboard/frontend/src/app/components/settings/SettingRow.tsx`
- Create: `crates/dashboard/frontend/src/app/components/settings/SettingSection.tsx`
- Create: `crates/dashboard/frontend/src/app/components/settings/controls.tsx`
- Create: `crates/dashboard/frontend/src/app/components/settings/index.ts`

**Step 1: Create the SettingRow component**

This is the core building block. Every setting renders as one row: label left, control right, optional description for obscure fields.

```tsx
// SettingRow.tsx
import { useState, useRef } from 'react';
import { HelpCircle } from 'lucide-react';

interface SettingRowProps {
  label: string;
  description?: string;
  tip?: string;
  children: React.ReactNode;
  /** Hides the bottom border (e.g. last item in a group) */
  last?: boolean;
}

export function SettingRow({ label, description, tip, children, last }: SettingRowProps) {
  return (
    <div
      className="flex items-center justify-between gap-4 px-4"
      style={{
        minHeight: description ? 60 : 44,
        borderBottom: last ? 'none' : '1px solid var(--codex-border-subtle)',
      }}
    >
      <div className="flex-1 min-w-0 py-2">
        <div className="flex items-center gap-1.5">
          <span className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
            {label}
          </span>
          {tip && <Tip text={tip} />}
        </div>
        {description && (
          <p className="text-[12px] mt-0.5" style={{ color: 'var(--codex-fg-subtle)' }}>
            {description}
          </p>
        )}
      </div>
      <div className="flex-shrink-0">{children}</div>
    </div>
  );
}

function Tip({ text }: { text: string }) {
  const [show, setShow] = useState(false);
  const ref = useRef<HTMLSpanElement>(null);
  return (
    <span
      ref={ref}
      className="inline-flex items-center relative"
      onMouseEnter={() => setShow(true)}
      onMouseLeave={() => setShow(false)}
    >
      <HelpCircle className="w-3.5 h-3.5 cursor-help" strokeWidth={1.5} style={{ color: '#555' }} />
      {show && (
        <span
          className="fixed z-[9999] px-3 py-2 rounded-lg text-[12px] leading-[1.6] w-[320px] shadow-xl pointer-events-none"
          style={{
            backgroundColor: '#1e1e1e',
            color: '#ccc',
            border: '1px solid #333',
            left: ref.current
              ? Math.min(ref.current.getBoundingClientRect().left, window.innerWidth - 340)
              : 0,
            top: ref.current ? ref.current.getBoundingClientRect().top - 8 : 0,
            transform: 'translateY(-100%)',
          }}
        >
          {text}
        </span>
      )}
    </span>
  );
}
```

**Step 2: Create the SettingSection component**

Collapsible sub-section with chevron toggle.

```tsx
// SettingSection.tsx
import { useState } from 'react';
import { ChevronDown, ChevronRight } from 'lucide-react';

interface SettingSectionProps {
  title: string;
  description?: string;
  defaultOpen?: boolean;
  children: React.ReactNode;
}

export function SettingSection({ title, description, defaultOpen = false, children }: SettingSectionProps) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div className="mb-2">
      <button
        onClick={() => setOpen(!open)}
        className="w-full flex items-center gap-2 px-4 py-2.5 text-left transition-colors rounded-lg"
        style={{ color: 'var(--codex-fg)' }}
        onMouseEnter={(e) => (e.currentTarget.style.backgroundColor = 'var(--codex-bg-secondary)')}
        onMouseLeave={(e) => (e.currentTarget.style.backgroundColor = 'transparent')}
      >
        {open ? (
          <ChevronDown className="w-4 h-4 flex-shrink-0" strokeWidth={1.5} style={{ color: 'var(--codex-fg-subtle)' }} />
        ) : (
          <ChevronRight className="w-4 h-4 flex-shrink-0" strokeWidth={1.5} style={{ color: 'var(--codex-fg-subtle)' }} />
        )}
        <div>
          <span className="text-[13px]" style={{ fontWeight: 500 }}>{title}</span>
          {description && !open && (
            <span className="text-[12px] ml-2" style={{ color: 'var(--codex-fg-subtle)' }}>{description}</span>
          )}
        </div>
      </button>
      {open && (
        <div
          className="rounded-lg overflow-hidden mx-1 mb-2"
          style={{ border: '1px solid var(--codex-border-subtle)', backgroundColor: 'var(--codex-bg-secondary)' }}
        >
          {children}
        </div>
      )}
    </div>
  );
}
```

**Step 3: Create control components**

All inline controls: Toggle, Select, NumberInput, TextInput, SecretInput, Slider, TagInput.

```tsx
// controls.tsx
import { useState } from 'react';
import { Eye, EyeOff, X, Plus } from 'lucide-react';

// ── Toggle ─────────────────────────────────────────────
export function Toggle({ checked, onChange, disabled }: {
  checked: boolean;
  onChange: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      onClick={disabled ? undefined : onChange}
      className="w-10 h-[22px] rounded-full relative transition-all flex-shrink-0"
      style={{
        backgroundColor: checked ? 'var(--codex-accent)' : '#333',
        opacity: disabled ? 0.5 : 1,
        cursor: disabled ? 'not-allowed' : 'pointer',
      }}
    >
      <div
        className="w-[18px] h-[18px] bg-white rounded-full absolute top-[2px] transition-all"
        style={{ left: checked ? '20px' : '2px' }}
      />
    </button>
  );
}

// ── Select ─────────────────────────────────────────────
export function Select({ value, options, onChange, placeholder, width }: {
  value: string;
  options: { value: string; label: string }[];
  onChange: (val: string) => void;
  placeholder?: string;
  width?: string;
}) {
  return (
    <select
      className="px-2.5 py-1.5 rounded border text-[13px] appearance-none bg-no-repeat"
      style={{
        backgroundColor: 'var(--codex-bg)',
        borderColor: 'var(--codex-border)',
        color: 'var(--codex-fg)',
        width: width ?? 'auto',
        maxWidth: 280,
        backgroundImage: `url("data:image/svg+xml,%3Csvg width='10' height='6' viewBox='0 0 10 6' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M1 1L5 5L9 1' stroke='%236b6b6b' stroke-width='1.5' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E")`,
        backgroundPosition: 'right 8px center',
        paddingRight: 28,
      }}
      value={value}
      onChange={(e) => onChange(e.target.value)}
    >
      {placeholder && <option value="">{placeholder}</option>}
      {options.map((o) => (
        <option key={o.value} value={o.value}>{o.label}</option>
      ))}
    </select>
  );
}

// ── NumberInput ────────────────────────────────────────
export function NumberInput({ value, onChange, min, max, step, suffix, width }: {
  value: number;
  onChange: (val: number) => void;
  min?: number;
  max?: number;
  step?: number;
  suffix?: string;
  width?: string;
}) {
  return (
    <div className="relative inline-flex items-center">
      <input
        type="number"
        defaultValue={value}
        key={value}
        min={min}
        max={max}
        step={step}
        className="px-2.5 py-1.5 rounded border text-[13px] outline-none tabular-nums"
        style={{
          backgroundColor: 'var(--codex-bg)',
          borderColor: 'var(--codex-border)',
          color: 'var(--codex-fg)',
          width: width ?? (suffix ? 100 : 80),
          paddingRight: suffix ? 32 : undefined,
        }}
        onChange={(e) => {
          const n = parseFloat(e.target.value);
          if (!Number.isNaN(n)) onChange(n);
        }}
      />
      {suffix && (
        <span className="absolute right-2.5 text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>
          {suffix}
        </span>
      )}
    </div>
  );
}

// ── TextInput ──────────────────────────────────────────
export function TextInput({ value, onChange, placeholder, disabled, width }: {
  value: string;
  onChange?: (val: string) => void;
  placeholder?: string;
  disabled?: boolean;
  width?: string;
}) {
  return (
    <input
      type="text"
      defaultValue={value}
      key={value}
      placeholder={placeholder}
      disabled={disabled}
      readOnly={!onChange}
      className="px-2.5 py-1.5 rounded border text-[13px] outline-none"
      style={{
        backgroundColor: 'var(--codex-bg)',
        borderColor: 'var(--codex-border)',
        color: disabled ? 'var(--codex-fg-subtle)' : 'var(--codex-fg)',
        width: width ?? 240,
        opacity: disabled ? 0.6 : 1,
      }}
      onChange={onChange ? (e) => onChange(e.target.value) : undefined}
    />
  );
}

// ── SecretInput ────────────────────────────────────────
export function SecretInput({ value, onChange, placeholder, width }: {
  value: string;
  onChange: (val: string) => void;
  placeholder?: string;
  width?: string;
}) {
  const [show, setShow] = useState(false);
  return (
    <div className="relative inline-flex items-center">
      <input
        type={show ? 'text' : 'password'}
        defaultValue={value}
        key={value}
        placeholder={placeholder}
        className="px-2.5 py-1.5 pr-8 rounded border text-[13px] outline-none"
        style={{
          backgroundColor: 'var(--codex-bg)',
          borderColor: 'var(--codex-border)',
          color: 'var(--codex-fg)',
          fontFamily: 'var(--font-mono)',
          width: width ?? 240,
        }}
        onChange={(e) => onChange(e.target.value)}
      />
      <button
        onClick={() => setShow(!show)}
        className="absolute right-1.5 p-1"
        style={{ color: 'var(--codex-fg-subtle)' }}
      >
        {show ? <EyeOff className="w-3.5 h-3.5" strokeWidth={1.5} /> : <Eye className="w-3.5 h-3.5" strokeWidth={1.5} />}
      </button>
    </div>
  );
}

// ── Slider ─────────────────────────────────────────────
export function Slider({ value, onChange, min, max, step }: {
  value: number;
  onChange: (val: number) => void;
  min: number;
  max: number;
  step: number;
}) {
  return (
    <div className="flex items-center gap-3">
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        className="w-28"
        onChange={(e) => onChange(parseFloat(e.target.value))}
      />
      <span className="text-[13px] tabular-nums w-10 text-right" style={{ color: 'var(--codex-accent)', fontFamily: 'var(--font-mono)' }}>
        {value}
      </span>
    </div>
  );
}

// ── TagInput ───────────────────────────────────────────
export function TagInput({ tags, onChange, placeholder }: {
  tags: string[];
  onChange: (tags: string[]) => void;
  placeholder?: string;
}) {
  const [input, setInput] = useState('');
  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {tags.map((tag) => (
        <span
          key={tag}
          className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-[12px]"
          style={{ backgroundColor: 'var(--codex-bg)', border: '1px solid var(--codex-border)', color: 'var(--codex-fg-muted)' }}
        >
          {tag}
          <button onClick={() => onChange(tags.filter((t) => t !== tag))} className="ml-0.5" style={{ color: 'var(--codex-fg-subtle)' }}>
            <X className="w-3 h-3" strokeWidth={2} />
          </button>
        </span>
      ))}
      <div className="inline-flex items-center gap-1">
        <input
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && input.trim()) {
              e.preventDefault();
              onChange([...tags, input.trim()]);
              setInput('');
            }
          }}
          placeholder={placeholder ?? 'Add...'}
          className="px-2 py-0.5 rounded border text-[12px] outline-none w-24"
          style={{ backgroundColor: 'var(--codex-bg)', borderColor: 'var(--codex-border)', color: 'var(--codex-fg)' }}
        />
      </div>
    </div>
  );
}

// ── TabStrip ───────────────────────────────────────────
export function TabStrip({ tabs, active, onChange }: {
  tabs: { id: string; label: string }[];
  active: string;
  onChange: (id: string) => void;
}) {
  return (
    <div className="flex gap-1 px-4 py-2 overflow-x-auto" style={{ borderBottom: '1px solid var(--codex-border-subtle)' }}>
      {tabs.map((tab) => {
        const isActive = tab.id === active;
        return (
          <button
            key={tab.id}
            onClick={() => onChange(tab.id)}
            className="px-3 py-1.5 text-[12px] rounded transition-colors whitespace-nowrap"
            style={{
              color: isActive ? 'var(--codex-accent)' : 'var(--codex-fg-subtle)',
              backgroundColor: isActive ? 'var(--codex-accent-dim)' : 'transparent',
              fontWeight: isActive ? 500 : 400,
            }}
          >
            {tab.label}
          </button>
        );
      })}
    </div>
  );
}

// ── TimeInput ──────────────────────────────────────────
export function TimeInput({ value, onChange }: {
  value: string;
  onChange: (val: string) => void;
}) {
  return (
    <input
      type="time"
      defaultValue={value}
      key={value}
      className="px-2.5 py-1.5 rounded border text-[13px] outline-none"
      style={{
        backgroundColor: 'var(--codex-bg)',
        borderColor: 'var(--codex-border)',
        color: 'var(--codex-fg)',
      }}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}
```

**Step 4: Create the barrel export**

```tsx
// index.ts
export { SettingRow } from './SettingRow';
export { SettingSection } from './SettingSection';
export {
  Toggle,
  Select,
  NumberInput,
  TextInput,
  SecretInput,
  Slider,
  TagInput,
  TabStrip,
  TimeInput,
} from './controls';
```

**Step 5: Verify TypeScript compiles**

Run: `cd crates/dashboard/frontend && npx tsc --noEmit`
Expected: No errors (new files are not imported yet, so they just need to type-check independently).

**Step 6: Commit**

```bash
git add crates/dashboard/frontend/src/app/components/settings/
git commit -m "feat(dashboard): add shared setting components for redesign"
```

---

### Task 2: Create Model Select Data + Constants

**Files:**
- Create: `crates/dashboard/frontend/src/app/components/settings/model-data.ts`

**Step 1: Create the known models map and display name helpers**

```tsx
// model-data.ts

/** Known models per provider for the smart model select. */
export const PROVIDER_MODELS: Record<string, string[]> = {
  anthropic: [
    'claude-opus-4-0520',
    'claude-sonnet-4-20250514',
    'claude-haiku-4-5-20251001',
  ],
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
  anthropic: 'Anthropic',
  openai: 'OpenAI',
  openrouter: 'OpenRouter',
  deepseek: 'DeepSeek',
  gemini: 'Gemini',
  groq: 'Groq',
  vllm: 'vLLM',
  zhipu: 'Zhipu',
  dashscope: 'DashScope',
  moonshot: 'Moonshot',
  minimax: 'MiniMax',
  aihubmix: 'AIHubMix',
  telegram: 'Telegram',
  discord: 'Discord',
  whatsapp: 'WhatsApp',
  slack: 'Slack',
  email: 'Email',
  qq: 'QQ',
  feishu: 'Feishu',
  dingtalk: 'DingTalk',
  mochat: 'Mochat',
};

export function displayName(key: string): string {
  return DISPLAY_NAMES[key] ?? key;
}

/** Common IANA timezones for the timezone select. */
export const TIMEZONES = [
  'UTC',
  'America/New_York',
  'America/Chicago',
  'America/Denver',
  'America/Los_Angeles',
  'America/Sao_Paulo',
  'Europe/London',
  'Europe/Paris',
  'Europe/Berlin',
  'Europe/Moscow',
  'Asia/Dubai',
  'Asia/Kolkata',
  'Asia/Bangkok',
  'Asia/Ho_Chi_Minh',
  'Asia/Shanghai',
  'Asia/Tokyo',
  'Asia/Seoul',
  'Asia/Singapore',
  'Australia/Sydney',
  'Pacific/Auckland',
];

export const CURRENCIES = ['USD', 'VND', 'EUR', 'GBP', 'JPY', 'KRW', 'CNY', 'THB', 'BTC', 'ETH', 'USDT'];
export const CURRENCY_SYMBOLS: Record<string, string> = {
  USD: '$', EUR: '€', GBP: '£', JPY: '¥', KRW: '₩', VND: '₫', CNY: '¥', THB: '฿', BTC: '₿',
};
```

**Step 2: Commit**

```bash
git add crates/dashboard/frontend/src/app/components/settings/model-data.ts
git commit -m "feat(dashboard): add model data and constants for settings"
```

---

### Task 3: Create the useExchangeRates Hook

**Files:**
- Create: `crates/dashboard/frontend/src/lib/hooks/useExchangeRates.ts`

**Step 1: Extract the exchange rate fetching logic from Settings.tsx**

Move the `fetchRates` / `liveRates` / `ratesTimestamp` state + logic from the Settings component into a standalone hook. This is currently embedded at Settings.tsx L:156-207.

```tsx
// useExchangeRates.ts
import { useState, useCallback } from 'react';

const CACHE_KEY = 'klyntbot_exchange_rates';
const CACHE_TTL = 3_600_000; // 1 hour

export interface UseExchangeRatesResult {
  rates: Record<string, number> | null;
  loading: boolean;
  timestamp: string | null;
  refresh: () => void;
}

export function useExchangeRates(): UseExchangeRatesResult {
  const [rates, setRates] = useState<Record<string, number> | null>(null);
  const [loading, setLoading] = useState(false);
  const [timestamp, setTimestamp] = useState<string | null>(null);

  const fetchRates = useCallback(async (force = false) => {
    setLoading(true);
    if (!force) {
      try {
        const cached = localStorage.getItem(CACHE_KEY);
        if (cached) {
          const parsed = JSON.parse(cached);
          if (Date.now() - parsed.fetchedAt < CACHE_TTL) {
            setRates(parsed.rates);
            setTimestamp(new Date(parsed.fetchedAt).toLocaleString());
            setLoading(false);
            return;
          }
        }
      } catch { /* ignore */ }
    }
    try {
      const fiatRes = await fetch('https://open.er-api.com/v6/latest/USD');
      const result: Record<string, number> = { USD: 1 };
      if (fiatRes.ok) {
        const data = await fiatRes.json();
        if (data.result === 'success' && data.rates) {
          for (const [cur, perUSD] of Object.entries(data.rates as Record<string, number>)) {
            if (cur !== 'USD' && perUSD > 0) result[cur] = 1 / perUSD;
          }
        }
      }
      try {
        const cryptoRes = await fetch(
          'https://api.coingecko.com/api/v3/simple/price?ids=bitcoin,ethereum,tether&vs_currencies=usd',
        );
        if (cryptoRes.ok) {
          const cd = await cryptoRes.json();
          if (cd.bitcoin?.usd) result.BTC = cd.bitcoin.usd;
          if (cd.ethereum?.usd) result.ETH = cd.ethereum.usd;
          if (cd.tether?.usd) result.USDT = cd.tether.usd;
        }
      } catch { /* crypto optional */ }
      setRates(result);
      const now = new Date();
      setTimestamp(now.toLocaleString());
      localStorage.setItem(CACHE_KEY, JSON.stringify({ rates: result, fetchedAt: now.getTime() }));
    } catch { /* silent */ }
    setLoading(false);
  }, []);

  const refresh = useCallback(() => fetchRates(true), [fetchRates]);

  return { rates, loading, timestamp, refresh };
}
```

**Step 2: Commit**

```bash
git add crates/dashboard/frontend/src/lib/hooks/useExchangeRates.ts
git commit -m "refactor(dashboard): extract exchange rate logic into useExchangeRates hook"
```

---

### Task 4: Rewrite Settings.tsx — Shell + General Section

**Files:**
- Modify: `crates/dashboard/frontend/src/app/pages/Settings.tsx` (complete rewrite)

**Step 1: Replace the entire Settings.tsx with the new shell**

This creates the page structure: 8-group left nav, no right sidebar, save indicator, config helpers, and the General section as the first implementation.

The file should include:
- Same `useApi<ConfigMap>('/api/settings')` fetch
- Same `patchSection` / `debouncedPatch` helpers
- Same `asRecord` / `str` / `num` / `bool` / `arr` helpers
- New 8-section nav with icons (General, AI & Models, Channels, Tools, Tasks, AI Behavior, Finance, Extensions)
- Save indicator as a subtle inline "Saved" / "Saving..." text near the page title (replaces the 260px right sidebar)
- General section: Timezone (select), Data Directory (read-only), Gateway Host (text), Gateway Port (number)

Key structure:
```tsx
export default function Settings() {
  // ... state, useApi, helpers (same as before)

  const sections = [
    { id: 'general', label: 'General', icon: Sliders },
    { id: 'ai-models', label: 'AI & Models', icon: Cpu },
    { id: 'channels', label: 'Channels', icon: MessageCircle },
    { id: 'tools', label: 'Tools', icon: Wrench },
    { id: 'tasks', label: 'Tasks', icon: CheckSquare },
    { id: 'ai-behavior', label: 'AI Behavior', icon: Brain },
    { id: 'finance', label: 'Finance', icon: DollarSign },
    { id: 'extensions', label: 'Extensions', icon: Package },
  ];

  return (
    <div className="flex-1 flex overflow-hidden">
      {/* Left Nav — 200px */}
      <nav className="w-[200px] border-r overflow-y-auto" ...>
        {sections.map(s => <button key={s.id} ...>{s.label}</button>)}
      </nav>

      {/* Content Area — flex-1, NO right sidebar */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Header with save indicator */}
        <div className="border-b px-6 py-4 flex items-center justify-between" ...>
          <div>
            <h1>{activeSection label}</h1>
            <p>{activeSection description}</p>
          </div>
          {/* Save indicator */}
          {saving && <span>Saving...</span>}
          {saveStatus && <span>{saveStatus.msg}</span>}
        </div>

        {/* Scrollable content */}
        <div className="flex-1 overflow-y-auto px-6 py-4">
          {activeSection === 'general' && <GeneralSection />}
          {activeSection === 'ai-models' && <AIModelsSection />}
          {/* ... */}
        </div>
      </div>
    </div>
  );
}
```

Each section will be its own inline function (or separate component if it exceeds ~100 lines) within Settings.tsx.

**Step 2: Implement General section using new components**

```tsx
// Inside Settings.tsx, or as a local function
function GeneralSection() {
  return (
    <>
      <SettingSection title="General" defaultOpen>
        <SettingRow label="Timezone" description="IANA timezone for scheduling and display">
          <Select
            value={str(config ?? {}, 'timezone', 'UTC')}
            options={TIMEZONES.map(tz => ({ value: tz, label: tz }))}
            onChange={(v) => {
              // timezone is top-level string
              apiFetch('/api/settings/timezone', { method: 'PATCH', body: v })
                .then(() => setConfig(prev => prev ? { ...prev, timezone: v } : prev))
                .catch(() => {});
            }}
          />
        </SettingRow>
        <SettingRow label="Data Directory" description="SQLite DB and LanceDB vectors location. Read-only.">
          <TextInput value={str(config ?? {}, 'dataDir', '~/.klyntbot')} disabled />
        </SettingRow>
        <SettingRow label="Gateway Host">
          <TextInput value={str(gateway, 'host', '127.0.0.1')} onChange={(v) => debouncedPatch('gateway', { host: v })} />
        </SettingRow>
        <SettingRow label="Gateway Port" description="Requires server restart" last>
          <NumberInput value={num(gateway, 'port', 18790)} onChange={(v) => debouncedPatch('gateway', { port: v })} />
        </SettingRow>
      </SettingSection>
    </>
  );
}
```

**Step 3: Verify build**

Run: `cd crates/dashboard/frontend && npx tsc --noEmit && npx vite build`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/dashboard/frontend/src/app/pages/Settings.tsx
git commit -m "feat(dashboard): rewrite Settings shell with 8-group nav and General section"
```

---

### Task 5: AI & Models Section

**Files:**
- Modify: `crates/dashboard/frontend/src/app/pages/Settings.tsx`

**Step 1: Implement the AI & Models section with 3 sub-sections**

Three sub-sections:
1. **Providers** (defaultOpen): TabStrip for provider selection → per-provider: API Key (SecretInput), API Base (TextInput), Native Mode (Toggle), Cache System Prompt (Toggle), Extended Thinking (Toggle + conditional Budget Tokens), API Version (TextInput)
2. **Agent Defaults** (collapsed): Provider (Select from providerTabs), Model (Select with PROVIDER_MODELS + "Custom..." option), Temperature (Slider 0-1), Max Tokens (NumberInput), Max Tool Iterations (NumberInput), Max Concurrent Subagents (NumberInput)
3. **Routing** (collapsed): Primary Provider (Select), Fallback Provider (Select), Classifier Model (TextInput)

For the Model select — use a standard `<Select>` with options from `PROVIDER_MODELS[selectedProvider]` plus a "Custom..." option. When "Custom..." is selected, show a TextInput below for freeform entry.

**Step 2: Verify build**

Run: `cd crates/dashboard/frontend && npx tsc --noEmit && npx vite build`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/dashboard/frontend/src/app/pages/Settings.tsx
git commit -m "feat(dashboard): add AI & Models section with providers, defaults, routing"
```

---

### Task 6: Channels Section

**Files:**
- Modify: `crates/dashboard/frontend/src/app/pages/Settings.tsx`

**Step 1: Implement Channels section**

Single section with TabStrip for channel selection. Per-channel rows:
- Enabled (Toggle)
- Token (SecretInput) — uses `botToken` or `token` depending on channel
- Allow From (TagInput)
- Proxy (TextInput) — only for Telegram

**Step 2: Verify + Commit**

```bash
cd crates/dashboard/frontend && npx tsc --noEmit && npx vite build
git commit -m "feat(dashboard): add Channels section"
```

---

### Task 7: Tools Section

**Files:**
- Modify: `crates/dashboard/frontend/src/app/pages/Settings.tsx`

**Step 1: Implement Tools section with 3 sub-sections**

1. **Web Search** (defaultOpen): Brave API Key (SecretInput), Max Results (NumberInput)
2. **Browser** (collapsed): Enabled (Toggle), Trust Level (Select: strict/autonomous/full), Session Timeout (NumberInput with "sec" suffix)
3. **Permissions** (collapsed): Restrict to Workspace (Toggle), Default Permission Level (Select: full/admin/elevated/standard/readOnly)

**Step 2: Verify + Commit**

```bash
cd crates/dashboard/frontend && npx tsc --noEmit && npx vite build
git commit -m "feat(dashboard): add Tools section"
```

---

### Task 8: Tasks Section

**Files:**
- Modify: `crates/dashboard/frontend/src/app/pages/Settings.tsx`

**Step 1: Implement Tasks section with 5 sub-sections**

1. **General** (defaultOpen): Creation Mode (Select: ask-first/yolo/party), Projects Enabled (Toggle)
2. **Enrichment** (collapsed): Enabled (Toggle), Auto Apply Threshold (NumberInput step=0.05), Use LLM (Toggle)
3. **Search** (collapsed): Semantic Search Enabled (Toggle), Semantic Threshold (NumberInput step=0.1), Embedding Model (TextInput), RRF K (NumberInput) — with descriptions for obscure fields
4. **Notifications** (collapsed): Targets (TagInput), Focus Reminders (Toggle), Daily Digest (Toggle), Digest Time (TimeInput)
5. **Focus & Planning** (collapsed): Max Slots (NumberInput), Deadline Hours (NumberInput), Daily Planning Enabled (Toggle), Planning Time (TimeInput)

**Step 2: Verify + Commit**

```bash
cd crates/dashboard/frontend && npx tsc --noEmit && npx vite build
git commit -m "feat(dashboard): add Tasks section"
```

---

### Task 9: AI Behavior Section

**Files:**
- Modify: `crates/dashboard/frontend/src/app/pages/Settings.tsx`

**Step 1: Implement AI Behavior section with 4 sub-sections**

1. **Conversation** (defaultOpen): Embedding Enabled (Toggle), Exclude Channels (TagInput), Exclude Roles (TagInput), Search Enabled (Toggle), Semantic Threshold (NumberInput), Max Results (NumberInput)
2. **Session** (collapsed): History Limit (NumberInput), TTL Days (NumberInput), Cleanup Interval Hours (NumberInput)
3. **Memory** (collapsed): Decay Half-Life Days (NumberInput, with description), Max Age Days (NumberInput), Consolidation (Toggle, with description), Maintenance Interval Hours (NumberInput)
4. **Learning & Confidence** (collapsed): Learning Enabled (Toggle), Analysis Interval (NumberInput with "sec" suffix, with description), Min/Max Threshold (NumberInput), Min Outcomes (NumberInput, with description), Confidence Enabled (Toggle), Confidence Threshold (Slider 0-1 step 0.05), Tool Overrides (key-value list)

**Step 2: Verify + Commit**

```bash
cd crates/dashboard/frontend && npx tsc --noEmit && npx vite build
git commit -m "feat(dashboard): add AI Behavior section"
```

---

### Task 10: Finance Section

**Files:**
- Modify: `crates/dashboard/frontend/src/app/pages/Settings.tsx`

**Step 1: Implement Finance section with 6 sub-sections**

Uses `useExchangeRates` hook instead of inline state.

1. **General** (defaultOpen): Enabled (Toggle), Display Currency (Select from CURRENCIES), Proactivity (Select: full/moderate/reactive)
2. **Budgeting** (collapsed): Default Method (Select: standard/six_jar), Alert Threshold % (NumberInput), Six Jar Ratios (6 NumberInputs in a compact grid, only visible when method is six_jar)
3. **Investment Returns** (collapsed): Stocks/Crypto/Real Estate/Bonds % (4 NumberInputs, with description)
4. **Inflation** (collapsed): Annual Rate % (NumberInput), Source (Select: manual/api)
5. **Auto-Categorization** (collapsed): Auto-Categorize (Toggle), Confidence Threshold (Slider 0-100)
6. **Scheduling** (collapsed): Daily Review Time (TimeInput), Budget Check Time (TimeInput), Weekly Report Day (Select: monday-sunday)

Note: Exchange Rates display is a special read-only section shown within Finance > General, using the `useExchangeRates` hook. Keep it compact — show rates in a grid with a refresh button.

**Step 2: Verify + Commit**

```bash
cd crates/dashboard/frontend && npx tsc --noEmit && npx vite build
git commit -m "feat(dashboard): add Finance section with useExchangeRates hook"
```

---

### Task 11: Extensions Section

**Files:**
- Modify: `crates/dashboard/frontend/src/app/pages/Settings.tsx`

**Step 1: Implement Extensions section with 3 sub-sections**

1. **Packs** (defaultOpen): Enabled Packs (read-only badge list, with note to use `klyntbot init --packs` to modify)
2. **Skills** (collapsed): Enabled Skills (TagInput)
3. **Plugins** (collapsed): Plugin System Enabled (Toggle), Registry URL (TextInput), Sandbox Memory MB (NumberInput), Allow Network by Default (Toggle)

**Step 2: Verify build**

Run: `cd crates/dashboard/frontend && npx tsc --noEmit && npx vite build`
Expected: PASS with no errors

**Step 3: Commit**

```bash
git add -A crates/dashboard/frontend/src/
git commit -m "feat(dashboard): add Extensions section, complete settings redesign"
```

---

### Task 12: Final Polish + Cleanup

**Files:**
- Modify: `crates/dashboard/frontend/src/app/pages/Settings.tsx` (if needed)

**Step 1: Run full build**

Run: `cd crates/dashboard/frontend && npx tsc --noEmit && npx vite build`
Expected: PASS

**Step 2: Verify all sections render**

- Check that all 8 sections switch correctly in the nav
- Check that all sub-sections expand/collapse
- Check that the old right sidebar is completely removed
- Verify no leftover references to old section IDs (providers, agent-defaults, tasks-todo, calendar, conversation, learning, confidence, projects, packs-skills, plugins)

**Step 3: Final commit if any remaining polish needed**

```bash
git commit -m "chore(dashboard): settings redesign cleanup"
```

---

## Key Implementation Notes

**Config path mapping** — these config paths from the old code must be preserved exactly:

| Section | Config path | Notes |
|---------|------------|-------|
| Timezone | `timezone` (top-level string) | Special: PATCH body is the string directly, not an object |
| Gateway | `gateway.host`, `gateway.port` | |
| Providers | `providers.<name>.apiKey`, etc. | Nested under provider name |
| Provider Manager | `providerManager.primary`, `.fallback`, `.classifierModel` | NOT `routing` |
| Agent Defaults | `agents.defaults.model`, `.provider`, etc. | Nested under `agents.defaults` |
| Channels | `channels.<name>.enabled`, `.token`/`.botToken`, `.allowFrom`, `.proxy` | Some use `botToken`, some use `token` |
| Tools | `tools.web.braveApiKey`, `tools.browser.*`, `tools.restrictToWorkspace`, `tools.permissions.defaultLevel` | |
| Todo | `todo.creationMode`, `todo.enrichment.*`, `todo.search.*`, `todo.notifications.*`, `todo.focus.*`, `todo.dailyPlanning.*` | |
| Calendar | `calendar.bidirectionalSync`, `calendar.conflictResolution`, `calendar.providers[]` | Providers is an array — rebuild whole array on patch |
| Conversation | `conversation.embedding.*`, `conversation.search.*`, `conversation.session.*`, `conversation.memory.*` | |
| Learning | `learning.enabled`, `learning.analysisIntervalSecs`, etc. | Top-level section |
| Confidence | `confidence.enabled`, `confidence.threshold`, `confidence.toolOverrides` | |
| Finance | `finance.enabled`, `finance.defaultCurrency`, etc. | Many nested sub-objects |
| Projects | `project.enabled` | NOTE: singular `project`, not `projects` |
| Packs | `packs.enabled`, `packs.enabledSkills` | |
| Plugins | `plugins.enabled`, `plugins.registryUrl`, etc. | |

**Debounce rules:**
- Toggle changes: immediate `patchSection()` (no debounce)
- Select changes: immediate `patchSection()`
- Text/number inputs: `debouncedPatch()` with 800ms delay
- Secret inputs (API keys, passwords): `debouncedPatch()` with 1200ms delay
