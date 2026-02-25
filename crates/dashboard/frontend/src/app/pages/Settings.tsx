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
  const [activeProvider, setActiveProvider] = useState('anthropic');
  const [activeChannel, setActiveChannel] = useState('telegram');
  const [showApiKey, setShowApiKey] = useState(false);
  const [customModel, setCustomModel] = useState(false);

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
  const providersMap = asRecord(config?.providers);
  const providerNames = Object.keys(providersMap);
  const providerTabs = providerNames.length > 0
    ? providerNames
    : ['anthropic', 'openai', 'openrouter', 'deepseek', 'gemini', 'groq', 'vllm', 'zhipu', 'dashscope', 'moonshot', 'minimax', 'aihubmix'];

  const channelsMap = asRecord(config?.channels);
  const channelNames = Object.keys(channelsMap);
  const channelTabs = channelNames.length > 0
    ? channelNames
    : ['telegram', 'discord', 'whatsapp', 'slack', 'email', 'qq', 'feishu', 'dingtalk', 'mochat'];

  const currentProviderConfig = asRecord(providersMap[activeProvider]);
  const currentChannelConfig = asRecord(channelsMap[activeChannel]);
  const agents = asRecord(config?.agents);
  const agentDefaults = asRecord(agents.defaults);
  const tools = asRecord(config?.tools);
  const toolsWeb = asRecord(tools.web);
  const toolsBrowser = asRecord(tools.browser);

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
              <>
                {/* Providers sub-section */}
                <SettingSection title="Providers" defaultOpen>
                  <TabStrip
                    tabs={providerTabs.map(p => ({ id: p, label: displayName(p) }))}
                    active={activeProvider}
                    onChange={setActiveProvider}
                  />
                  <SettingRow label="API Key">
                    <SecretInput
                      value={str(currentProviderConfig, 'apiKey', '')}
                      onChange={(v) => debouncedPatch('providers', { [activeProvider]: { apiKey: v } }, 1200)}
                      width="220px"
                    />
                  </SettingRow>
                  <SettingRow label="API Base URL" description="Custom endpoint (leave empty for default)"
                    tip="Override the default API endpoint. Useful for proxies, self-hosted models, or region-specific endpoints.">
                    <TextInput
                      value={str(currentProviderConfig, 'apiBase', '')}
                      placeholder={`https://api.${activeProvider}.com`}
                      onChange={(v) => debouncedPatch('providers', { [activeProvider]: { apiBase: v || null } })}
                    />
                  </SettingRow>
                  <SettingRow label="Native Mode"
                    tip="Send requests using the provider's native API format rather than converting through a unified schema.">
                    <Toggle
                      checked={bool(currentProviderConfig, 'native', false)}
                      onChange={() => patchSection('providers', { [activeProvider]: { native: !bool(currentProviderConfig, 'native', false) } })}
                    />
                  </SettingRow>
                  <SettingRow label="Cache System Prompt"
                    tip="Caches the system prompt on the provider side to reduce token usage on repeated calls. Supported by Anthropic.">
                    <Toggle
                      checked={bool(currentProviderConfig, 'cacheSystemPrompt', false)}
                      onChange={() => patchSection('providers', { [activeProvider]: { cacheSystemPrompt: !bool(currentProviderConfig, 'cacheSystemPrompt', false) } })}
                    />
                  </SettingRow>
                  <SettingRow label="Extended Thinking"
                    tip="Gives the model a dedicated thinking budget to reason through complex problems before responding.">
                    <Toggle
                      checked={bool(asRecord(currentProviderConfig.extendedThinking), 'enabled', false)}
                      onChange={() => patchSection('providers', { [activeProvider]: { extendedThinking: { enabled: !bool(asRecord(currentProviderConfig.extendedThinking), 'enabled', false) } } })}
                    />
                  </SettingRow>
                  {bool(asRecord(currentProviderConfig.extendedThinking), 'enabled', false) && (
                    <SettingRow label="Budget Tokens" description="Max tokens for internal reasoning (5,000-50,000)"
                      tip="Higher = deeper thinking but more expensive.">
                      <NumberInput
                        value={num(asRecord(currentProviderConfig.extendedThinking), 'budgetTokens', 10000)}
                        onChange={(v) => debouncedPatch('providers', { [activeProvider]: { extendedThinking: { budgetTokens: v } } })}
                      />
                    </SettingRow>
                  )}
                  <SettingRow label="API Version" description="Provider-specific version (e.g. 2023-06-01)"
                    tip="Only set if you need a specific version for beta features." last>
                    <TextInput
                      value={str(currentProviderConfig, 'apiVersion', '')}
                      placeholder="2023-06-01"
                      onChange={(v) => debouncedPatch('providers', { [activeProvider]: { apiVersion: v || null } })}
                      width="160px"
                    />
                  </SettingRow>
                </SettingSection>

                {/* Agent Defaults sub-section */}
                <SettingSection title="Agent Defaults">
                  <SettingRow label="Provider">
                    <Select
                      value={str(agentDefaults, 'provider', '')}
                      options={providerTabs.map(p => ({ value: p, label: displayName(p) }))}
                      placeholder="Auto-detect from model"
                      onChange={(v) => patchSection('agents', { defaults: { provider: v || null } })}
                    />
                  </SettingRow>
                  <SettingRow label="Model">
                    {customModel ? (
                      <TextInput
                        value={str(agentDefaults, 'model', '')}
                        placeholder="model-name"
                        onChange={(v) => debouncedPatch('agents', { defaults: { model: v } })}
                        width="200px"
                      />
                    ) : (
                      <Select
                        value={str(agentDefaults, 'model', '')}
                        options={[
                          ...(PROVIDER_MODELS[str(agentDefaults, 'provider', 'anthropic')] ?? []).map(m => ({ value: m, label: m })),
                          { value: '__custom__', label: 'Custom...' },
                        ]}
                        onChange={(v) => {
                          if (v === '__custom__') { setCustomModel(true); return; }
                          patchSection('agents', { defaults: { model: v } });
                        }}
                      />
                    )}
                  </SettingRow>
                  <SettingRow label="Temperature" description="0 = deterministic, 1 = creative">
                    <Slider
                      value={num(agentDefaults, 'temperature', 0.7)}
                      onChange={(v) => patchSection('agents', { defaults: { temperature: v } })}
                      min={0} max={1} step={0.1}
                    />
                  </SettingRow>
                  <SettingRow label="Max Tokens"
                    tip="Maximum output tokens per LLM response. Most tasks work fine with 4096-8192.">
                    <NumberInput
                      value={num(agentDefaults, 'maxTokens', 8192)}
                      onChange={(v) => debouncedPatch('agents', { defaults: { maxTokens: v } })}
                    />
                  </SettingRow>
                  <SettingRow label="Max Tool Iterations"
                    tip="Safety limit to prevent runaway tool loops.">
                    <NumberInput
                      value={num(agentDefaults, 'maxToolIterations', 20)}
                      onChange={(v) => debouncedPatch('agents', { defaults: { maxToolIterations: v } })}
                    />
                  </SettingRow>
                  <SettingRow label="Max Concurrent Subagents" last>
                    <NumberInput
                      value={num(agentDefaults, 'maxConcurrentSubagents', 3)}
                      onChange={(v) => debouncedPatch('agents', { defaults: { maxConcurrentSubagents: v } })}
                    />
                  </SettingRow>
                </SettingSection>

                {/* Routing sub-section */}
                <SettingSection title="Routing">
                  <SettingRow label="Primary Provider">
                    <Select
                      value={str(asRecord(config?.providerManager), 'primary', '')}
                      options={providerTabs.map(p => ({ value: p, label: displayName(p) }))}
                      placeholder="Auto-detect"
                      onChange={(v) => patchSection('providerManager', { primary: v || null })}
                    />
                  </SettingRow>
                  <SettingRow label="Fallback Provider">
                    <Select
                      value={str(asRecord(config?.providerManager), 'fallback', '')}
                      options={providerTabs.map(p => ({ value: p, label: displayName(p) }))}
                      placeholder="None"
                      onChange={(v) => patchSection('providerManager', { fallback: v || null })}
                    />
                  </SettingRow>
                  <SettingRow label="Classifier Model" description="Lightweight model for routing decisions"
                    tip="A fast, cheap model that decides which provider/model to use for each request." last>
                    <TextInput
                      value={str(asRecord(config?.providerManager), 'classifierModel', '')}
                      placeholder="gpt-4o-mini"
                      onChange={(v) => debouncedPatch('providerManager', { classifierModel: v || null })}
                      width="180px"
                    />
                  </SettingRow>
                </SettingSection>
              </>
            )}

            {/* ── CHANNELS ─────────────────────────────────────────── */}
            {activeSection === 'channels' && (
              <SettingSection title="Channels" defaultOpen>
                <TabStrip
                  tabs={channelTabs.map(c => ({ id: c, label: displayName(c) }))}
                  active={activeChannel}
                  onChange={setActiveChannel}
                />
                <SettingRow label="Enabled">
                  <Toggle
                    checked={bool(currentChannelConfig, 'enabled', false)}
                    onChange={() => patchSection('channels', { [activeChannel]: { enabled: !bool(currentChannelConfig, 'enabled', false) } })}
                  />
                </SettingRow>
                <SettingRow label="Token">
                  <SecretInput
                    value={str(currentChannelConfig, 'token', str(currentChannelConfig, 'botToken', ''))}
                    onChange={(v) => {
                      const key = currentChannelConfig.botToken !== undefined ? 'botToken' : 'token';
                      debouncedPatch('channels', { [activeChannel]: { [key]: v } }, 1200);
                    }}
                    width="220px"
                  />
                </SettingRow>
                <SettingRow label="Allow From" description="Permitted user/chat IDs">
                  <TagInput
                    tags={arr(currentChannelConfig, 'allowFrom').map(String)}
                    onChange={(tags) => patchSection('channels', { [activeChannel]: { allowFrom: tags } })}
                    placeholder="Add ID..."
                  />
                </SettingRow>
                {(activeChannel === 'telegram' || str(currentChannelConfig, 'proxy', '') !== '') && (
                  <SettingRow label="Proxy" description="SOCKS5/HTTP proxy URL" last>
                    <TextInput
                      value={str(currentChannelConfig, 'proxy', '')}
                      placeholder="socks5://127.0.0.1:1080"
                      onChange={(v) => debouncedPatch('channels', { [activeChannel]: { proxy: v || null } })}
                    />
                  </SettingRow>
                )}
              </SettingSection>
            )}

            {/* ── TOOLS ────────────────────────────────────────────── */}
            {activeSection === 'tools' && (
              <>
                <SettingSection title="Web Search" defaultOpen>
                  <SettingRow label="Brave API Key">
                    <SecretInput
                      value={str(toolsWeb, 'braveApiKey', '')}
                      onChange={(v) => debouncedPatch('tools', { web: { braveApiKey: v } }, 1200)}
                      width="220px"
                    />
                  </SettingRow>
                  <SettingRow label="Max Results" last>
                    <NumberInput
                      value={num(toolsWeb, 'maxResults', 5)}
                      onChange={(v) => debouncedPatch('tools', { web: { maxResults: v } })}
                    />
                  </SettingRow>
                </SettingSection>

                <SettingSection title="Browser Automation">
                  <SettingRow label="Enabled">
                    <Toggle
                      checked={bool(toolsBrowser, 'enabled', false)}
                      onChange={() => patchSection('tools', { browser: { enabled: !bool(toolsBrowser, 'enabled', false) } })}
                    />
                  </SettingRow>
                  <SettingRow label="Trust Level"
                    tip="Strict: asks before every action. Autonomous: asks only for dangerous actions. Full: no confirmations.">
                    <Select
                      value={str(toolsBrowser, 'trustLevel', 'autonomous')}
                      options={[
                        { value: 'strict', label: 'Strict' },
                        { value: 'autonomous', label: 'Autonomous' },
                        { value: 'full', label: 'Full' },
                      ]}
                      onChange={(v) => patchSection('tools', { browser: { trustLevel: v } })}
                    />
                  </SettingRow>
                  <SettingRow label="Session Timeout" last>
                    <NumberInput
                      value={num(toolsBrowser, 'sessionTimeoutSecs', 300)}
                      onChange={(v) => debouncedPatch('tools', { browser: { sessionTimeoutSecs: v } })}
                      suffix="sec"
                    />
                  </SettingRow>
                </SettingSection>

                <SettingSection title="Permissions">
                  <SettingRow label="Restrict to Workspace"
                    tip="When enabled, file tools can only access files within the workspace directory.">
                    <Toggle
                      checked={bool(tools, 'restrictToWorkspace', false)}
                      onChange={() => patchSection('tools', { restrictToWorkspace: !bool(tools, 'restrictToWorkspace', false) })}
                    />
                  </SettingRow>
                  <SettingRow label="Default Permission Level"
                    tip="Controls which tools the agent can use. You can override per-channel." last>
                    <Select
                      value={str(asRecord(tools.permissions), 'defaultLevel', 'standard')}
                      options={[
                        { value: 'full', label: 'Full' },
                        { value: 'admin', label: 'Admin' },
                        { value: 'elevated', label: 'Elevated' },
                        { value: 'standard', label: 'Standard' },
                        { value: 'readOnly', label: 'Read Only' },
                      ]}
                      onChange={(v) => patchSection('tools', { permissions: { defaultLevel: v } })}
                    />
                  </SettingRow>
                </SettingSection>
              </>
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
