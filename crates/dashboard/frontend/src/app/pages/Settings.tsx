import { useState, useCallback, useRef } from 'react';
import {
  Sliders, Cpu, MessageCircle, Wrench, CheckSquare,
  Brain, DollarSign, Package, Loader2, AlertTriangle, RefreshCw,
} from 'lucide-react';
import { useApi } from '../../lib/hooks/useApi';
import { apiFetch } from '../../lib/api';
import { SettingRow } from '../../app/components/settings/SettingRow';
import { SettingSection } from '../../app/components/settings/SettingSection';
import {
  Toggle, Select, NumberInput, TextInput, SecretInput,
  Slider, TagInput, TabStrip, TimeInput,
} from '../../app/components/settings/controls';
import {
  PROVIDER_MODELS, displayName, TIMEZONES, CURRENCIES, CURRENCY_SYMBOLS,
} from '../../app/components/settings/model-data';

// ── Type helpers ──────────────────────────────────────────────────────────────

type ConfigMap = Record<string, unknown>;

function asRecord(val: unknown): Record<string, unknown> {
  if (val && typeof val === 'object' && !Array.isArray(val)) return val as Record<string, unknown>;
  return {};
}

function str(rec: Record<string, unknown>, key: string, fallback = ''): string {
  const v = rec[key];
  if (typeof v === 'string') return v;
  if (typeof v === 'number' || typeof v === 'boolean') return String(v);
  return fallback;
}

function num(rec: Record<string, unknown>, key: string, fallback = 0): number {
  const v = rec[key];
  if (typeof v === 'number') return v;
  if (typeof v === 'string') { const n = Number(v); if (!Number.isNaN(n)) return n; }
  return fallback;
}

function bool(rec: Record<string, unknown>, key: string, fallback = false): boolean {
  const v = rec[key];
  if (typeof v === 'boolean') return v;
  return fallback;
}

function arr(rec: Record<string, unknown>, key: string): unknown[] {
  const v = rec[key];
  if (Array.isArray(v)) return v;
  return [];
}

// ── Section definitions ───────────────────────────────────────────────────────

type SectionDef = { id: string; label: string; icon: typeof Sliders; desc: string };

const SECTIONS: SectionDef[] = [
  { id: 'general', label: 'General', icon: Sliders, desc: 'Application settings' },
  { id: 'ai-models', label: 'AI & Models', icon: Cpu, desc: 'Providers, model defaults, and routing' },
  { id: 'channels', label: 'Channels', icon: MessageCircle, desc: 'Chat platform integrations' },
  { id: 'tools', label: 'Tools', icon: Wrench, desc: 'Tool permissions and configuration' },
  { id: 'tasks', label: 'Tasks', icon: CheckSquare, desc: 'Task management and productivity' },
  { id: 'ai-behavior', label: 'AI Behavior', icon: Brain, desc: 'Conversation, memory, and learning' },
  { id: 'finance', label: 'Finance', icon: DollarSign, desc: 'Financial tracking and budgets' },
  { id: 'extensions', label: 'Extensions', icon: Package, desc: 'Packs, skills, and plugins' },
];

// ── Main component ────────────────────────────────────────────────────────────

export default function Settings() {
  const [activeSection, setActiveSection] = useState('general');

  const { data: config, loading, error, refetch, setData: setConfig } = useApi<ConfigMap>('/api/settings');

  // ── Save helpers ──────────────────────────────────────────────────────────
  const [saving, setSaving] = useState(false);
  const [saveStatus, setSaveStatus] = useState<{ ok: boolean; msg: string } | null>(null);
  const saveTimerRef = useRef<ReturnType<typeof setTimeout>>();

  const patchSection = useCallback(async (section: string, patch: Record<string, unknown>) => {
    setSaving(true);
    setSaveStatus(null);
    try {
      await apiFetch(`/api/settings/${section}`, { method: 'PATCH', body: patch });
      setSaveStatus({ ok: true, msg: 'Saved' });
      setConfig(prev => {
        if (!prev) return prev;
        const sectionData = (prev[section] ?? {}) as Record<string, unknown>;
        return { ...prev, [section]: { ...sectionData, ...patch } };
      });
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : 'Failed to save';
      setSaveStatus({ ok: false, msg });
    } finally {
      setSaving(false);
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
      saveTimerRef.current = setTimeout(() => setSaveStatus(null), 3000);
    }
  }, [setConfig]);

  const debounceTimerRef = useRef<ReturnType<typeof setTimeout>>();
  const debouncedPatch = useCallback((section: string, patch: Record<string, unknown>, delay = 800) => {
    if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
    debounceTimerRef.current = setTimeout(() => patchSection(section, patch), delay);
  }, [patchSection]);

  // ── Config section accessors ──────────────────────────────────────────────
  const gateway = asRecord(config?.gateway);

  // ── Loading / error states ────────────────────────────────────────────────
  if (loading && !config) {
    return (
      <div className="flex-1 flex items-center justify-center" style={{ backgroundColor: 'var(--codex-bg)' }}>
        <div className="flex flex-col items-center gap-3">
          <Loader2 className="w-6 h-6 animate-spin" style={{ color: 'var(--codex-accent)' }} />
          <span className="text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>Loading settings...</span>
        </div>
      </div>
    );
  }

  if (error && !config) {
    return (
      <div className="flex-1 flex items-center justify-center" style={{ backgroundColor: 'var(--codex-bg)' }}>
        <div className="flex flex-col items-center gap-3 max-w-sm text-center">
          <AlertTriangle className="w-6 h-6" style={{ color: '#e5534b' }} />
          <span className="text-[13px]" style={{ color: 'var(--codex-fg)' }}>Failed to load settings</span>
          <span className="text-[12px]" style={{ color: '#666' }}>{error.message}</span>
          <button onClick={refetch} className="flex items-center gap-2 px-4 py-2 rounded text-[13px] mt-2"
            style={{ backgroundColor: 'var(--codex-accent)', color: 'white' }}>
            <RefreshCw className="w-3.5 h-3.5" strokeWidth={1.5} /> Retry
          </button>
        </div>
      </div>
    );
  }

  const currentSection = SECTIONS.find(s => s.id === activeSection)!;

  return (
    <div className="flex-1 flex overflow-hidden">
      {/* Left Settings Nav */}
      <nav className="w-[200px] border-r overflow-y-auto flex-shrink-0" style={{
        backgroundColor: 'var(--codex-bg-secondary)',
        borderColor: 'var(--codex-border-subtle)',
      }}>
        <div className="p-3">
          <h2 className="text-[10px] uppercase tracking-wider mb-3 px-3" style={{
            color: 'var(--codex-fg-subtle)', fontWeight: 500,
          }}>Settings</h2>
          <div className="space-y-0.5">
            {SECTIONS.map((s) => {
              const isActive = activeSection === s.id;
              return (
                <button key={s.id} onClick={() => setActiveSection(s.id)}
                  className="w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-[13px] transition-all relative"
                  style={{
                    color: isActive ? 'var(--codex-accent)' : 'var(--codex-fg-subtle)',
                    backgroundColor: isActive ? 'var(--codex-accent-dim)' : 'transparent',
                  }}
                  onMouseEnter={(e) => { if (!isActive) e.currentTarget.style.backgroundColor = 'var(--codex-bg)'; }}
                  onMouseLeave={(e) => { if (!isActive) e.currentTarget.style.backgroundColor = 'transparent'; }}
                >
                  {isActive && (
                    <div className="absolute left-0 top-1/2 -translate-y-1/2 w-[2px] h-4 rounded-r"
                      style={{ backgroundColor: 'var(--codex-accent)' }} />
                  )}
                  <s.icon className="w-4 h-4" strokeWidth={1.5} />
                  {s.label}
                </button>
              );
            })}
          </div>
        </div>
      </nav>

      {/* Content Area */}
      <div className="flex-1 flex flex-col overflow-hidden" style={{ backgroundColor: 'var(--codex-bg)' }}>
        {/* Header */}
        <div className="border-b px-6 py-4 flex items-center justify-between flex-shrink-0" style={{
          borderColor: 'var(--codex-border-subtle)',
        }}>
          <div>
            <h1 className="text-lg" style={{ color: 'var(--codex-fg)', fontWeight: 400 }}>
              {currentSection.label}
            </h1>
            <p className="text-[12px] mt-0.5" style={{ color: 'var(--codex-fg-subtle)' }}>
              {currentSection.desc}
            </p>
          </div>
          {/* Save indicator */}
          <div className="flex items-center gap-2 text-[12px]">
            {saving && (
              <>
                <Loader2 className="w-3.5 h-3.5 animate-spin" style={{ color: 'var(--codex-accent)' }} strokeWidth={1.5} />
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Saving...</span>
              </>
            )}
            {saveStatus && !saving && (
              <span style={{ color: saveStatus.ok ? 'var(--codex-accent)' : '#ef4444' }}>
                {saveStatus.msg}
              </span>
            )}
          </div>
        </div>

        {/* Scrollable content */}
        <div className="flex-1 overflow-y-auto px-6 py-4">
          <div className="max-w-2xl">

            {/* ── GENERAL ──────────────────────────────────────────── */}
            {activeSection === 'general' && (
              <SettingSection title="General" defaultOpen>
                <SettingRow label="Timezone" description="IANA timezone for scheduling and display">
                  <Select
                    value={str(config ?? {}, 'timezone', 'UTC')}
                    options={TIMEZONES.map(tz => ({ value: tz, label: tz }))}
                    onChange={(v) => {
                      apiFetch('/api/settings/timezone', { method: 'PATCH', body: v })
                        .then(() => setConfig(prev => prev ? { ...prev, timezone: v } : prev))
                        .catch(() => {});
                    }}
                  />
                </SettingRow>
                <SettingRow label="Data Directory" description="SQLite DB and LanceDB vectors. Read-only.">
                  <TextInput value={str(config ?? {}, 'dataDir', '~/.klyntbot')} disabled />
                </SettingRow>
                <SettingRow label="Gateway Host">
                  <TextInput value={str(gateway, 'host', '127.0.0.1')}
                    onChange={(v) => debouncedPatch('gateway', { host: v })} />
                </SettingRow>
                <SettingRow label="Gateway Port" description="Requires server restart" last>
                  <NumberInput value={num(gateway, 'port', 18790)}
                    onChange={(v) => debouncedPatch('gateway', { port: v })} />
                </SettingRow>
              </SettingSection>
            )}

            {/* ── AI & MODELS ──────────────────────────────────────── */}
            {activeSection === 'ai-models' && (
              <p className="text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>Coming soon</p>
            )}

            {/* ── CHANNELS ─────────────────────────────────────────── */}
            {activeSection === 'channels' && (
              <p className="text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>Coming soon</p>
            )}

            {/* ── TOOLS ────────────────────────────────────────────── */}
            {activeSection === 'tools' && (
              <p className="text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>Coming soon</p>
            )}

            {/* ── TASKS ────────────────────────────────────────────── */}
            {activeSection === 'tasks' && (
              <p className="text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>Coming soon</p>
            )}

            {/* ── AI BEHAVIOR ──────────────────────────────────────── */}
            {activeSection === 'ai-behavior' && (
              <p className="text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>Coming soon</p>
            )}

            {/* ── FINANCE ──────────────────────────────────────────── */}
            {activeSection === 'finance' && (
              <p className="text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>Coming soon</p>
            )}

            {/* ── EXTENSIONS ───────────────────────────────────────── */}
            {activeSection === 'extensions' && (
              <p className="text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>Coming soon</p>
            )}

          </div>
        </div>
      </div>
    </div>
  );
}
