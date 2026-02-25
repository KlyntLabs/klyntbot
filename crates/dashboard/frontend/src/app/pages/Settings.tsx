import { useState, useCallback, useRef } from 'react';
import {
  Sliders, Cpu, MessageCircle, Wrench, CheckSquare,
  Brain, DollarSign, Package, Loader2, AlertTriangle, RefreshCw, X,
} from 'lucide-react';
import { useApi } from '../../lib/hooks/useApi';
import { useExchangeRates } from '../../lib/hooks/useExchangeRates';
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
  const exchangeRates = useExchangeRates();

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
  const todo = asRecord(config?.todo);
  const enrichment = asRecord(todo.enrichment);
  const search = asRecord(todo.search);
  const todoNotifications = asRecord(todo.notifications);
  const todoFocus = asRecord(todo.focus);
  const todoDailyPlanning = asRecord(todo.dailyPlanning);
  const projects = asRecord(config?.project);
  const conversation = asRecord(config?.conversation);
  const conversationEmbedding = asRecord(conversation.embedding);
  const conversationSearch = asRecord(conversation.search);
  const conversationSession = asRecord(conversation.session);
  const conversationMemory = asRecord(conversation.memory);
  const learning = asRecord(config?.learning);
  const confidence = asRecord(config?.confidence);
  const finance = asRecord(config?.finance);
  const financeInflation = asRecord(finance.inflation);
  const financeExpectedReturns = asRecord(finance.expectedReturns);
  const financeBudgeting = asRecord(finance.budgeting);
  const sixJarRatios = asRecord(financeBudgeting.sixJarRatios);
  const financeCategories = asRecord(finance.categories);
  const financePriceRefresh = asRecord(finance.priceRefresh);
  const financeScheduling = asRecord(finance.scheduling);
  const packs = asRecord(config?.packs);
  const plugins = asRecord(config?.plugins);

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
              <>
                <SettingSection title="General" defaultOpen>
                  <SettingRow label="Creation Mode" description="How the agent handles task creation">
                    <Select
                      value={str(todo, 'creationMode', 'ask-first')}
                      options={[
                        { value: 'ask-first', label: 'Ask First' },
                        { value: 'yolo', label: 'Yolo' },
                        { value: 'party', label: 'Party' },
                      ]}
                      onChange={(v) => patchSection('todo', { creationMode: v })}
                    />
                  </SettingRow>
                  <SettingRow label="Projects Enabled" last>
                    <Toggle
                      checked={bool(projects, 'enabled', true)}
                      onChange={() => patchSection('project', { enabled: !bool(projects, 'enabled', true) })}
                    />
                  </SettingRow>
                </SettingSection>

                <SettingSection title="Enrichment" description="Auto-infer missing task fields">
                  <SettingRow label="Enabled">
                    <Toggle
                      checked={bool(enrichment, 'enabled', true)}
                      onChange={() => patchSection('todo', { enrichment: { enabled: !bool(enrichment, 'enabled', true) } })}
                    />
                  </SettingRow>
                  <SettingRow label="Auto Apply Threshold" description="Confidence threshold for auto-applying suggestions (0-1)"
                    tip="Only suggestions above this threshold are automatically applied. Lower = more auto-fills.">
                    <NumberInput
                      value={num(enrichment, 'autoApplyThreshold', 0.85)}
                      onChange={(v) => debouncedPatch('todo', { enrichment: { autoApplyThreshold: v } })}
                      step={0.05} min={0} max={1}
                    />
                  </SettingRow>
                  <SettingRow label="Use LLM" description="Use an LLM for richer enrichment instead of keyword matching" last>
                    <Toggle
                      checked={bool(enrichment, 'useLlm', false)}
                      onChange={() => patchSection('todo', { enrichment: { useLlm: !bool(enrichment, 'useLlm', false) } })}
                    />
                  </SettingRow>
                </SettingSection>

                <SettingSection title="Search" description="Semantic search for task retrieval">
                  <SettingRow label="Semantic Search">
                    <Toggle
                      checked={bool(search, 'enabled', true)}
                      onChange={() => patchSection('todo', { search: { enabled: !bool(search, 'enabled', true) } })}
                    />
                  </SettingRow>
                  <SettingRow label="Semantic Threshold" description="Minimum cosine similarity (0-1)"
                    tip="Controls how similar results must be to your query. Lower for broader matches.">
                    <NumberInput
                      value={num(search, 'semanticThreshold', 0.5)}
                      onChange={(v) => debouncedPatch('todo', { search: { semanticThreshold: v } })}
                      step={0.1} min={0} max={1}
                    />
                  </SettingRow>
                  <SettingRow label="Embedding Model"
                    tip="Local model for vector embeddings. Supports 50+ languages.">
                    <TextInput
                      value={str(search, 'embeddingModel', 'paraphrase-multilingual-MiniLM-L12-v2')}
                      onChange={(v) => debouncedPatch('todo', { search: { embeddingModel: v } })}
                    />
                  </SettingRow>
                  <SettingRow label="RRF K" description="Reciprocal Rank Fusion parameter for hybrid search"
                    tip="Controls blending of keyword and semantic results. K=60 balances both signals." last>
                    <NumberInput
                      value={num(search, 'rrfK', 60)}
                      onChange={(v) => debouncedPatch('todo', { search: { rrfK: v } })}
                    />
                  </SettingRow>
                </SettingSection>

                <SettingSection title="Notifications">
                  <SettingRow label="Targets" description="Where to send notifications">
                    <TagInput
                      tags={arr(todoNotifications, 'targets').map(String)}
                      onChange={(tags) => patchSection('todo', { notifications: { targets: tags } })}
                      placeholder="os_native"
                    />
                  </SettingRow>
                  <SettingRow label="Focus Reminders">
                    <Toggle
                      checked={bool(todoNotifications, 'focusReminders', true)}
                      onChange={() => patchSection('todo', { notifications: { focusReminders: !bool(todoNotifications, 'focusReminders', true) } })}
                    />
                  </SettingRow>
                  <SettingRow label="Daily Digest">
                    <Toggle
                      checked={bool(todoNotifications, 'dailyDigest', true)}
                      onChange={() => patchSection('todo', { notifications: { dailyDigest: !bool(todoNotifications, 'dailyDigest', true) } })}
                    />
                  </SettingRow>
                  <SettingRow label="Digest Time" last>
                    <TimeInput
                      value={str(todoNotifications, 'dailyDigestTime', '09:00')}
                      onChange={(v) => debouncedPatch('todo', { notifications: { dailyDigestTime: v } })}
                    />
                  </SettingRow>
                </SettingSection>

                <SettingSection title="Focus & Planning">
                  <SettingRow label="Max Slots" description="Maximum tasks in the focus queue">
                    <NumberInput
                      value={num(todoFocus, 'maxSlots', 3)}
                      onChange={(v) => debouncedPatch('todo', { focus: { maxSlots: v } })}
                    />
                  </SettingRow>
                  <SettingRow label="Deadline Hours" description="Hours before deadline when reminders begin"
                    tip="Set to 18 for morning-of reminders on tasks due that day.">
                    <NumberInput
                      value={num(todoFocus, 'deadlineHours', 18)}
                      onChange={(v) => debouncedPatch('todo', { focus: { deadlineHours: v } })}
                    />
                  </SettingRow>
                  <SettingRow label="Daily Planning">
                    <Toggle
                      checked={bool(todoDailyPlanning, 'enabled', true)}
                      onChange={() => patchSection('todo', { dailyPlanning: { enabled: !bool(todoDailyPlanning, 'enabled', true) } })}
                    />
                  </SettingRow>
                  <SettingRow label="Planning Time" last>
                    <TimeInput
                      value={str(todoDailyPlanning, 'planningTime', '08:00')}
                      onChange={(v) => debouncedPatch('todo', { dailyPlanning: { planningTime: v } })}
                    />
                  </SettingRow>
                </SettingSection>
              </>
            )}

            {/* ── AI BEHAVIOR ──────────────────────────────────────── */}
            {activeSection === 'ai-behavior' && (
              <>
                <SettingSection title="Conversation" defaultOpen>
                  <SettingRow label="Embedding Enabled" description="Store conversation embeddings for semantic recall">
                    <Toggle
                      checked={bool(conversationEmbedding, 'enabled', true)}
                      onChange={() => patchSection('conversation', { embedding: { enabled: !bool(conversationEmbedding, 'enabled', true) } })}
                    />
                  </SettingRow>
                  <SettingRow label="Exclude Channels" description="Channels to skip for embeddings">
                    <TagInput
                      tags={arr(conversationEmbedding, 'excludeChannels').map(String)}
                      onChange={(tags) => patchSection('conversation', { embedding: { excludeChannels: tags } })}
                    />
                  </SettingRow>
                  <SettingRow label="Exclude Roles">
                    <TagInput
                      tags={arr(conversationEmbedding, 'excludeRoles').map(String)}
                      onChange={(tags) => patchSection('conversation', { embedding: { excludeRoles: tags } })}
                      placeholder="system"
                    />
                  </SettingRow>
                  <SettingRow label="Search Enabled">
                    <Toggle
                      checked={bool(conversationSearch, 'enabled', true)}
                      onChange={() => patchSection('conversation', { search: { enabled: !bool(conversationSearch, 'enabled', true) } })}
                    />
                  </SettingRow>
                  <SettingRow label="Semantic Threshold"
                    tip="Same concept as todo search threshold — controls similarity for conversation search.">
                    <NumberInput
                      value={num(conversationSearch, 'semanticThreshold', 0.5)}
                      onChange={(v) => debouncedPatch('conversation', { search: { semanticThreshold: v } })}
                      step={0.1} min={0} max={1}
                    />
                  </SettingRow>
                  <SettingRow label="Max Results" last>
                    <NumberInput
                      value={num(conversationSearch, 'maxResults', 20)}
                      onChange={(v) => debouncedPatch('conversation', { search: { maxResults: v } })}
                    />
                  </SettingRow>
                </SettingSection>

                <SettingSection title="Session">
                  <SettingRow label="History Limit" description="Max messages kept in session context">
                    <NumberInput
                      value={num(conversationSession, 'historyLimit', 50)}
                      onChange={(v) => debouncedPatch('conversation', { session: { historyLimit: v } })}
                    />
                  </SettingRow>
                  <SettingRow label="TTL" suffix="days" description="Days before inactive sessions expire">
                    <NumberInput
                      value={num(conversationSession, 'ttlDays', 30)}
                      onChange={(v) => debouncedPatch('conversation', { session: { ttlDays: v } })}
                      suffix="days"
                    />
                  </SettingRow>
                  <SettingRow label="Cleanup Interval" last>
                    <NumberInput
                      value={num(conversationSession, 'cleanupIntervalHours', 1)}
                      onChange={(v) => debouncedPatch('conversation', { session: { cleanupIntervalHours: v } })}
                      suffix="hrs"
                    />
                  </SettingRow>
                </SettingSection>

                <SettingSection title="Memory" description="Long-term memory with time-based decay">
                  <SettingRow label="Decay Half-Life" description="Days for memory relevance to halve"
                    tip="Memories lose relevance over time using exponential decay. 138 days means 50% relevance after ~4.5 months.">
                    <NumberInput
                      value={num(conversationMemory, 'decayHalfLifeDays', 138)}
                      onChange={(v) => debouncedPatch('conversation', { memory: { decayHalfLifeDays: v } })}
                      suffix="days"
                    />
                  </SettingRow>
                  <SettingRow label="Max Age">
                    <NumberInput
                      value={num(conversationMemory, 'maxAgeDays', 90)}
                      onChange={(v) => debouncedPatch('conversation', { memory: { maxAgeDays: v } })}
                      suffix="days"
                    />
                  </SettingRow>
                  <SettingRow label="Consolidation" description="Merge similar memories to reduce redundancy"
                    tip="Periodically merges similar memories into a single summary. Reduces storage and prevents duplicate context.">
                    <Toggle
                      checked={bool(conversationMemory, 'consolidationEnabled', false)}
                      onChange={() => patchSection('conversation', { memory: { consolidationEnabled: !bool(conversationMemory, 'consolidationEnabled', false) } })}
                    />
                  </SettingRow>
                  <SettingRow label="Maintenance Interval" last>
                    <NumberInput
                      value={num(conversationMemory, 'maintenanceIntervalHours', 24)}
                      onChange={(v) => debouncedPatch('conversation', { memory: { maintenanceIntervalHours: v } })}
                      suffix="hrs"
                    />
                  </SettingRow>
                </SettingSection>

                <SettingSection title="Learning & Confidence">
                  <SettingRow label="Learning Enabled" description="Adapt strategies based on outcome analysis">
                    <Toggle
                      checked={bool(learning, 'enabled', true)}
                      onChange={() => patchSection('learning', { enabled: !bool(learning, 'enabled', true) })}
                    />
                  </SettingRow>
                  <SettingRow label="Analysis Interval" description="How often to analyze outcomes"
                    tip="The agent reviews past outcomes to learn which strategies work. Default 3600s (1 hour).">
                    <NumberInput
                      value={num(learning, 'analysisIntervalSecs', 3600)}
                      onChange={(v) => debouncedPatch('learning', { analysisIntervalSecs: v })}
                      suffix="sec"
                    />
                  </SettingRow>
                  <SettingRow label="Min Threshold" description="Minimum success rate before deprioritizing"
                    tip="Strategies below this rate get deprioritized.">
                    <NumberInput
                      value={num(learning, 'minThreshold', 0.4)}
                      onChange={(v) => debouncedPatch('learning', { minThreshold: v })}
                      step={0.1} min={0} max={1}
                    />
                  </SettingRow>
                  <SettingRow label="Max Threshold">
                    <NumberInput
                      value={num(learning, 'maxThreshold', 0.9)}
                      onChange={(v) => debouncedPatch('learning', { maxThreshold: v })}
                      step={0.1} min={0} max={1}
                    />
                  </SettingRow>
                  <SettingRow label="Min Outcomes" description="Outcomes required before adapting"
                    tip="Prevents premature adaptation based on too few data points.">
                    <NumberInput
                      value={num(learning, 'minOutcomesForAdaptation', 50)}
                      onChange={(v) => debouncedPatch('learning', { minOutcomesForAdaptation: v })}
                    />
                  </SettingRow>
                  <SettingRow label="Confidence Enabled" description="Require minimum confidence before executing tool calls">
                    <Toggle
                      checked={bool(confidence, 'enabled', true)}
                      onChange={() => patchSection('confidence', { enabled: !bool(confidence, 'enabled', true) })}
                    />
                  </SettingRow>
                  <SettingRow label="Confidence Threshold"
                    tip="Below this threshold, the agent asks for confirmation instead of executing.">
                    <Slider
                      value={num(confidence, 'threshold', 0.7)}
                      onChange={(v) => patchSection('confidence', { threshold: v })}
                      min={0} max={1} step={0.05}
                    />
                  </SettingRow>
                  <SettingRow label="Tool Overrides" description="Per-tool confidence thresholds"
                    tip="Set higher thresholds for dangerous tools and lower for safe ones." last>
                    <div className="flex flex-col gap-1.5">
                      {Object.entries(asRecord(confidence.toolOverrides)).map(([tool, threshold]) => (
                        <div key={tool} className="flex items-center gap-1.5">
                          <span className="text-[12px]" style={{ color: 'var(--codex-fg-muted)' }}>{tool}</span>
                          <span className="text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>{String(threshold)}</span>
                          <button
                            onClick={() => patchSection('confidence', { toolOverrides: { [tool]: null } })}
                            style={{ color: 'var(--codex-fg-subtle)' }}
                          >
                            <span className="text-[11px]">✕</span>
                          </button>
                        </div>
                      ))}
                      {Object.keys(asRecord(confidence.toolOverrides)).length === 0 && (
                        <span className="text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>No overrides</span>
                      )}
                    </div>
                  </SettingRow>
                </SettingSection>
              </>
            )}

            {/* ── FINANCE ──────────────────────────────────────────── */}
            {activeSection === 'finance' && (() => {
              // Load exchange rates when finance section is shown
              if (!exchangeRates.rates && !exchangeRates.loading) exchangeRates.load();
              const defaultCur = str(finance, 'defaultCurrency', 'USD');
              const rateDisplayCurrencies = ['USD', 'VND', 'EUR', 'GBP', 'JPY', 'BTC', 'ETH', 'USDT'].filter(c => c !== defaultCur);
              const formatRate = (from: string) => {
                if (!exchangeRates.rates) return '—';
                const fromR = exchangeRates.rates[from] ?? 1;
                const toR = exchangeRates.rates[defaultCur] ?? 1;
                const r = fromR / toR;
                if (r >= 100) return r.toLocaleString('en-US', { maximumFractionDigits: 0 });
                if (r >= 1) return r.toLocaleString('en-US', { maximumFractionDigits: 2 });
                if (r >= 0.01) return r.toLocaleString('en-US', { maximumFractionDigits: 4 });
                return r.toLocaleString('en-US', { maximumFractionDigits: 8 });
              };

              return (
              <>
                <SettingSection title="General" defaultOpen>
                  <SettingRow label="Finance Module">
                    <Toggle
                      checked={bool(finance, 'enabled', false)}
                      onChange={() => patchSection('finance', { enabled: !bool(finance, 'enabled', false) })}
                    />
                  </SettingRow>
                  <SettingRow label="Display Currency">
                    <Select
                      value={str(finance, 'defaultCurrency', 'USD')}
                      options={CURRENCIES.map(c => ({ value: c, label: `${c}${CURRENCY_SYMBOLS[c] ? ` (${CURRENCY_SYMBOLS[c]})` : ''}` }))}
                      onChange={(v) => patchSection('finance', { defaultCurrency: v })}
                    />
                  </SettingRow>
                  <SettingRow label="Proactivity" last>
                    <Select
                      value={str(finance, 'proactivityLevel', 'full')}
                      options={[
                        { value: 'full', label: 'Full' },
                        { value: 'moderate', label: 'Moderate' },
                        { value: 'reactive', label: 'Reactive' },
                      ]}
                      onChange={(v) => patchSection('finance', { proactivityLevel: v })}
                    />
                  </SettingRow>
                </SettingSection>

                {/* Exchange rates display */}
                {exchangeRates.rates && (
                  <div className="mx-1 mb-2 px-4 py-3 rounded-lg" style={{ backgroundColor: 'var(--codex-bg-secondary)', border: '1px solid var(--codex-border-subtle)' }}>
                    <div className="flex items-center justify-between mb-2">
                      <span className="text-[12px]" style={{ color: 'var(--codex-fg-subtle)', fontWeight: 500 }}>Exchange Rates (1 {defaultCur})</span>
                      <button onClick={exchangeRates.refresh} className="text-[11px] px-2 py-0.5 rounded"
                        style={{ color: 'var(--codex-fg-subtle)', border: '1px solid var(--codex-border)' }}>
                        {exchangeRates.loading ? 'Refreshing...' : 'Refresh'}
                      </button>
                    </div>
                    <div className="grid grid-cols-4 gap-x-4 gap-y-1">
                      {rateDisplayCurrencies.map(cur => (
                        <div key={cur} className="flex justify-between text-[12px]">
                          <span style={{ color: 'var(--codex-fg-subtle)' }}>{cur}</span>
                          <span style={{ color: 'var(--codex-fg-muted)', fontFamily: 'var(--font-mono)' }}>{formatRate(cur)}</span>
                        </div>
                      ))}
                    </div>
                    {exchangeRates.timestamp && (
                      <p className="text-[10px] mt-2" style={{ color: '#555' }}>Updated: {exchangeRates.timestamp}</p>
                    )}
                  </div>
                )}

                <SettingSection title="Budgeting">
                  <SettingRow label="Default Method">
                    <Select
                      value={str(financeBudgeting, 'defaultMethod', 'standard')}
                      options={[
                        { value: 'standard', label: 'Standard (envelope)' },
                        { value: 'six_jar', label: 'Six Jar Method' },
                      ]}
                      onChange={(v) => patchSection('finance', { budgeting: { defaultMethod: v } })}
                    />
                  </SettingRow>
                  <SettingRow label="Alert Threshold"
                    tip="Warn when spending exceeds this % of budget.">
                    <NumberInput
                      value={num(financeBudgeting, 'alertThreshold', 80)}
                      onChange={(v) => debouncedPatch('finance', { budgeting: { alertThreshold: v } })}
                      suffix="%"
                    />
                  </SettingRow>
                  {str(financeBudgeting, 'defaultMethod', 'standard') === 'six_jar' && (
                    <SettingRow label="Six Jar Ratios" description="Income allocation percentages (should sum to 100%)"
                      tip="T. Harv Eker's method: Essentials 55%, Savings 10%, Investment 10%, Education 10%, Play 10%, Charity 5%." last>
                      <div className="grid grid-cols-3 gap-1.5">
                        {[
                          { key: 'essentials', label: 'Ess', def: 55 },
                          { key: 'savings', label: 'Sav', def: 10 },
                          { key: 'investment', label: 'Inv', def: 10 },
                          { key: 'education', label: 'Edu', def: 10 },
                          { key: 'entertainment', label: 'Play', def: 10 },
                          { key: 'charity', label: 'Give', def: 5 },
                        ].map(({ key, label, def }) => (
                          <div key={key} className="flex items-center gap-1">
                            <span className="text-[10px] w-7" style={{ color: 'var(--codex-fg-subtle)' }}>{label}</span>
                            <NumberInput
                              value={num(sixJarRatios, key, def)}
                              onChange={(v) => debouncedPatch('finance', { budgeting: { sixJarRatios: { [key]: v } } })}
                              width="50px" suffix="%"
                            />
                          </div>
                        ))}
                      </div>
                    </SettingRow>
                  )}
                </SettingSection>

                <SettingSection title="Investment Returns" description="Default return assumptions for projections">
                  {[
                    { key: 'stocks', label: 'Stocks', def: 10 },
                    { key: 'crypto', label: 'Crypto', def: 15 },
                    { key: 'realEstate', label: 'Real Estate', def: 8 },
                    { key: 'bonds', label: 'Bonds', def: 5 },
                  ].map(({ key, label, def }, i, a) => (
                    <SettingRow key={key} label={label} last={i === a.length - 1}>
                      <NumberInput
                        value={num(financeExpectedReturns, key, def)}
                        onChange={(v) => debouncedPatch('finance', { expectedReturns: { [key]: v } })}
                        suffix="%" step={0.5}
                      />
                    </SettingRow>
                  ))}
                </SettingSection>

                <SettingSection title="Inflation">
                  <SettingRow label="Annual Rate">
                    <NumberInput
                      value={num(financeInflation, 'rate', 3.3)}
                      onChange={(v) => debouncedPatch('finance', { inflation: { rate: v } })}
                      suffix="%" step={0.1}
                    />
                  </SettingRow>
                  <SettingRow label="Source" last>
                    <Select
                      value={str(financeInflation, 'source', 'manual')}
                      options={[
                        { value: 'manual', label: 'Manual' },
                        { value: 'api', label: 'API (auto-fetch)' },
                      ]}
                      onChange={(v) => patchSection('finance', { inflation: { source: v } })}
                    />
                  </SettingRow>
                </SettingSection>

                <SettingSection title="Auto-Categorization">
                  <SettingRow label="Auto-Categorize" description="AI-powered automatic transaction categorization">
                    <Toggle
                      checked={bool(financeCategories, 'autoCategorize', true)}
                      onChange={() => patchSection('finance', { categories: { autoCategorize: !bool(financeCategories, 'autoCategorize', true) } })}
                    />
                  </SettingRow>
                  <SettingRow label="Confidence Threshold" description="Only auto-apply above this confidence" last>
                    <Slider
                      value={Math.round(num(financeCategories, 'confidenceThreshold', 0.8) * 100)}
                      onChange={(v) => patchSection('finance', { categories: { confidenceThreshold: v / 100 } })}
                      min={0} max={100} step={5}
                    />
                  </SettingRow>
                </SettingSection>

                <SettingSection title="Scheduling">
                  <SettingRow label="Daily Review Time">
                    <TimeInput
                      value={str(financeScheduling, 'dailyReviewTime', '21:00')}
                      onChange={(v) => debouncedPatch('finance', { scheduling: { dailyReviewTime: v } })}
                    />
                  </SettingRow>
                  <SettingRow label="Budget Check Time">
                    <TimeInput
                      value={str(financeScheduling, 'budgetCheckTime', '09:00')}
                      onChange={(v) => debouncedPatch('finance', { scheduling: { budgetCheckTime: v } })}
                    />
                  </SettingRow>
                  <SettingRow label="Weekly Report Day" last>
                    <Select
                      value={str(financeScheduling, 'weeklyReportDay', 'monday')}
                      options={['monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday', 'sunday'].map(d => ({
                        value: d, label: d.charAt(0).toUpperCase() + d.slice(1),
                      }))}
                      onChange={(v) => patchSection('finance', { scheduling: { weeklyReportDay: v } })}
                    />
                  </SettingRow>
                </SettingSection>
              </>
              );
            })()}

            {/* ── EXTENSIONS ───────────────────────────────────────── */}
            {activeSection === 'extensions' && (
              <>
                <SettingSection title="Feature Packs" defaultOpen>
                  <SettingRow label="Enabled Packs" description="Manage via klyntbot init --packs" last>
                    <div className="flex flex-wrap gap-1.5">
                      {arr(packs, 'enabled').length > 0
                        ? arr(packs, 'enabled').map(String).map(pack => (
                          <span key={pack} className="px-2 py-0.5 rounded text-[11px]"
                            style={{ backgroundColor: 'var(--codex-accent-dim)', color: 'var(--codex-accent)', border: '1px solid var(--codex-accent)' }}>
                            {pack}
                          </span>
                        ))
                        : <span className="text-[12px]" style={{ color: 'var(--codex-fg-subtle)' }}>No packs enabled</span>
                      }
                    </div>
                  </SettingRow>
                </SettingSection>

                <SettingSection title="Skills">
                  <SettingRow label="Enabled Skills" last>
                    <TagInput
                      tags={arr(packs, 'enabledSkills').map(String)}
                      onChange={(skills) => debouncedPatch('packs', { enabledSkills: skills })}
                      placeholder="Add skill..."
                    />
                  </SettingRow>
                </SettingSection>

                <SettingSection title="Plugins">
                  <SettingRow label="Plugin System">
                    <Toggle
                      checked={bool(plugins, 'enabled', true)}
                      onChange={() => patchSection('plugins', { enabled: !bool(plugins, 'enabled', true) })}
                    />
                  </SettingRow>
                  <SettingRow label="Registry URL">
                    <TextInput
                      value={str(plugins, 'registryUrl', 'https://plugins.klyntbot.io/index.json')}
                      onChange={(v) => debouncedPatch('plugins', { registryUrl: v })}
                    />
                  </SettingRow>
                  <SettingRow label="Sandbox Memory"
                    tip="Max memory each plugin sandbox can use. Exceeded plugins are terminated.">
                    <NumberInput
                      value={num(plugins, 'sandboxMemoryMb', 64)}
                      onChange={(v) => debouncedPatch('plugins', { sandboxMemoryMb: v })}
                      suffix="MB"
                    />
                  </SettingRow>
                  <SettingRow label="Allow Network by Default" description="Grant network access to plugins unless explicitly denied" last>
                    <Toggle
                      checked={bool(plugins, 'allowNetworkByDefault', false)}
                      onChange={() => patchSection('plugins', { allowNetworkByDefault: !bool(plugins, 'allowNetworkByDefault', false) })}
                    />
                  </SettingRow>
                </SettingSection>
              </>
            )}

          </div>
        </div>
      </div>
    </div>
  );
}
