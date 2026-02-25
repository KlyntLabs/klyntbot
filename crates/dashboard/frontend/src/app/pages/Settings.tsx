import { useState, useCallback, useRef, useEffect } from 'react';
import {
  Sliders,
  Cpu,
  MessageCircle,
  Bot,
  Wrench,
  CheckSquare,
  Calendar as CalendarIcon,
  MessageSquare,
  Brain,
  DollarSign,
  Folder,
  Package,
  Eye,
  EyeOff,
  ChevronDown,
  ChevronRight,
  Plus,
  X,
  Plug,
  Loader2,
  AlertTriangle,
  RefreshCw,
} from 'lucide-react';
import { useApi } from '../../lib/hooks/useApi';
import { apiFetch } from '../../lib/api';

type SettingsSection = {
  id: string;
  label: string;
  icon: typeof Sliders;
};

// ── Type helpers for accessing nested config sections ────────────────────────

type ConfigMap = Record<string, unknown>;

/** Safely cast an unknown config section to a flat record. */
function asRecord(val: unknown): Record<string, unknown> {
  if (val && typeof val === 'object' && !Array.isArray(val)) {
    return val as Record<string, unknown>;
  }
  return {};
}

/** Safely get a string value from a record, with an optional fallback. */
function str(rec: Record<string, unknown>, key: string, fallback = ''): string {
  const v = rec[key];
  if (typeof v === 'string') return v;
  if (typeof v === 'number' || typeof v === 'boolean') return String(v);
  return fallback;
}

/** Safely get a number value from a record, with an optional fallback. */
function num(rec: Record<string, unknown>, key: string, fallback = 0): number {
  const v = rec[key];
  if (typeof v === 'number') return v;
  if (typeof v === 'string') {
    const n = Number(v);
    if (!Number.isNaN(n)) return n;
  }
  return fallback;
}

/** Safely get a boolean value from a record. */
function bool(rec: Record<string, unknown>, key: string, fallback = false): boolean {
  const v = rec[key];
  if (typeof v === 'boolean') return v;
  return fallback;
}

/** Safely get an array value from a record. */
function arr(rec: Record<string, unknown>, key: string): unknown[] {
  const v = rec[key];
  if (Array.isArray(v)) return v;
  return [];
}

export default function Settings() {
  const [activeSection, setActiveSection] = useState('providers');
  const [activeProvider, setActiveProvider] = useState('anthropic');
  const [showApiKey, setShowApiKey] = useState(false);
  const [sessionOpen, setSessionOpen] = useState(true);
  const [tokenOpen, setTokenOpen] = useState(true);
  const [configOpen, setConfigOpen] = useState(true);

  const { data: config, loading, error, refetch, setData: setConfig } = useApi<ConfigMap>('/api/settings');

  // ── PATCH helper for saving settings ──────────────────────────────────────
  const [saving, setSaving] = useState(false);
  const [saveStatus, setSaveStatus] = useState<{ ok: boolean; msg: string } | null>(null);
  const saveTimerRef = useRef<ReturnType<typeof setTimeout>>();

  const patchSection = useCallback(async (section: string, patch: Record<string, unknown>) => {
    setSaving(true);
    setSaveStatus(null);
    try {
      await apiFetch(`/api/settings/${section}`, { method: 'PATCH', body: patch });
      setSaveStatus({ ok: true, msg: 'Saved' });
      // Optimistically merge the patch into local config
      // (GET /api/settings reads in-memory state which doesn't update after PATCH)
      setConfig(prev => {
        if (!prev) return prev;
        const sectionData = (prev[section] ?? {}) as Record<string, unknown>;
        return { ...prev, [section]: { ...sectionData, ...patch } };
      });
    } catch (e: any) {
      setSaveStatus({ ok: false, msg: e.message ?? 'Failed to save' });
    } finally {
      setSaving(false);
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
      saveTimerRef.current = setTimeout(() => setSaveStatus(null), 3000);
    }
  }, [setConfig]);

  // Debounced patch for text inputs
  const debounceTimerRef = useRef<ReturnType<typeof setTimeout>>();
  const debouncedPatch = useCallback((section: string, patch: Record<string, unknown>, delay = 800) => {
    if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
    debounceTimerRef.current = setTimeout(() => patchSection(section, patch), delay);
  }, [patchSection]);

  // ── Exchange rate state (for finance section) ──────────────────────────────
  const [liveRates, setLiveRates] = useState<Record<string, number> | null>(null);
  const [ratesLoadingState, setRatesLoadingState] = useState(false);
  const [ratesTimestamp, setRatesTimestamp] = useState<string | null>(null);

  const fetchRates = useCallback(async (force = false) => {
    setRatesLoadingState(true);
    if (!force) {
      try {
        const cached = localStorage.getItem('klyntbot_exchange_rates');
        if (cached) {
          const parsed = JSON.parse(cached);
          if (Date.now() - parsed.fetchedAt < 3_600_000) {
            setLiveRates(parsed.rates);
            setRatesTimestamp(new Date(parsed.fetchedAt).toLocaleString());
            setRatesLoadingState(false);
            return;
          }
        }
      } catch { /* ignore */ }
    }
    try {
      const fiatRes = await fetch('https://open.er-api.com/v6/latest/USD');
      const rates: Record<string, number> = { USD: 1 };
      if (fiatRes.ok) {
        const data = await fiatRes.json();
        if (data.result === 'success' && data.rates) {
          for (const [cur, perUSD] of Object.entries(data.rates as Record<string, number>)) {
            if (cur !== 'USD' && perUSD > 0) rates[cur] = 1 / perUSD;
          }
        }
      }
      try {
        const cryptoRes = await fetch('https://api.coingecko.com/api/v3/simple/price?ids=bitcoin,ethereum,tether&vs_currencies=usd');
        if (cryptoRes.ok) {
          const cd = await cryptoRes.json();
          if (cd.bitcoin?.usd) rates.BTC = cd.bitcoin.usd;
          if (cd.ethereum?.usd) rates.ETH = cd.ethereum.usd;
          if (cd.tether?.usd) rates.USDT = cd.tether.usd;
        }
      } catch { /* crypto optional */ }
      setLiveRates(rates);
      const now = new Date();
      setRatesTimestamp(now.toLocaleString());
      localStorage.setItem('klyntbot_exchange_rates', JSON.stringify({ rates, fetchedAt: now.getTime() }));
    } catch { /* silent */ }
    setRatesLoadingState(false);
  }, []);

  // Load rates when finance section is active
  useEffect(() => {
    if (activeSection === 'finance' && !liveRates) fetchRates();
  }, [activeSection, liveRates, fetchRates]);

  const sections: SettingsSection[] = [
    { id: 'general', label: 'General', icon: Sliders },
    { id: 'providers', label: 'Providers', icon: Cpu },
    { id: 'channels', label: 'Channels', icon: MessageCircle },
    { id: 'agent-defaults', label: 'Agent Defaults', icon: Bot },
    { id: 'tools', label: 'Tools', icon: Wrench },
    { id: 'tasks-todo', label: 'Tasks & Todo', icon: CheckSquare },
    { id: 'calendar', label: 'Calendar', icon: CalendarIcon },
    { id: 'conversation', label: 'Conversation', icon: MessageSquare },
    { id: 'learning', label: 'Learning', icon: Brain },
    { id: 'confidence', label: 'Confidence', icon: Brain },
    { id: 'finance', label: 'Finance', icon: DollarSign },
    { id: 'projects', label: 'Projects', icon: Folder },
    { id: 'packs-skills', label: 'Packs & Skills', icon: Package },
    { id: 'plugins', label: 'Plugins', icon: Plug },
  ];

  // ── Derive provider/channel lists from config ──────────────────────────────

  const providersMap = asRecord(config?.providers);
  const providerNames = Object.keys(providersMap);
  // Fallback list if config hasn't loaded yet
  const providerTabs = providerNames.length > 0
    ? providerNames
    : ['anthropic', 'openai', 'openrouter', 'deepseek', 'gemini', 'groq', 'vllm', 'zhipu', 'dashscope', 'moonshot', 'minimax', 'aihubmix'];

  const channelsMap = asRecord(config?.channels);
  const channelNames = Object.keys(channelsMap);
  const channelTabs = channelNames.length > 0
    ? channelNames
    : ['telegram', 'discord', 'whatsapp', 'slack', 'email', 'qq', 'feishu', 'dingtalk', 'mochat'];

  const [activeChannel, setActiveChannel] = useState('telegram');

  // ── Read config sections ───────────────────────────────────────────────────

  const gateway = asRecord(config?.gateway);
  const agents = asRecord(config?.agents);
  const agentDefaults = asRecord(agents.defaults);
  const tools = asRecord(config?.tools);
  const todo = asRecord(config?.todo);
  const enrichment = asRecord(todo.enrichment);
  const search = asRecord(todo.search);
  const todoNotifications = asRecord(todo.notifications);
  const todoFocus = asRecord(todo.focus);
  const todoDailyPlanning = asRecord(todo.dailyPlanning);
  const calendar = asRecord(config?.calendar);
  const calendarProviders = arr(calendar, 'providers');
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
  const financePriceRefresh = asRecord(finance.priceRefresh);
  const financeScheduling = asRecord(finance.scheduling);
  const financeCategories = asRecord(finance.categories);
  const sixJarRatios = asRecord(financeBudgeting.sixJarRatios);
  const projects = asRecord(config?.projects);
  const packs = asRecord(config?.packs);
  const plugins = asRecord(config?.plugins);
  const toolsWeb = asRecord(tools.web);
  const toolsBrowser = asRecord(tools.browser);

  // Current provider/channel data
  const currentProviderConfig = asRecord(providersMap[activeProvider]);
  const currentChannelConfig = asRecord(channelsMap[activeChannel]);

  // ── Display helper: format a provider name for display ─────────────────────

  function displayName(key: string): string {
    // Capitalize well-known provider/channel names
    const map: Record<string, string> = {
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
    return map[key] ?? key;
  }

  const FormCard = ({ children, title, description }: any) => (
    <div className="p-5 rounded-lg mb-4" style={{
      backgroundColor: '#141414',
      border: '1px solid var(--codex-border)'
    }}>
      {title && (
        <>
          <label className="text-[13px] block mb-1" style={{
            color: 'var(--codex-fg)',
            fontWeight: 500
          }}>
            {title}
          </label>
          {description && (
            <p className="text-[12px] mb-3" style={{ color: '#666' }}>
              {description}
            </p>
          )}
        </>
      )}
      {children}
    </div>
  );

  const TextInput = ({ value, placeholder, secret = false }: any) => (
    <input
      type={secret ? 'password' : 'text'}
      defaultValue={value}
      placeholder={placeholder}
      className="w-full px-3 py-2 rounded border outline-none text-[13px]"
      style={{
        backgroundColor: 'var(--codex-bg)',
        borderColor: 'var(--codex-border)',
        color: 'var(--codex-fg)',
        fontFamily: secret ? 'var(--font-mono)' : 'var(--font-ui)'
      }}
      readOnly
      // TODO: Remove readOnly and add onChange handler that debounces + PATCHes
    />
  );

  const Toggle = ({ checked, onChange }: any) => (
    <button
      onClick={onChange}
      className="w-11 h-6 rounded-full relative transition-all flex-shrink-0"
      style={{
        backgroundColor: checked ? 'var(--codex-accent)' : '#333'
      }}
    >
      <div className="w-5 h-5 bg-white rounded-full absolute top-0.5 transition-all" style={{
        left: checked ? '22px' : '2px'
      }} />
    </button>
  );

  // ── Loading state ──────────────────────────────────────────────────────────

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

  // ── Error state ────────────────────────────────────────────────────────────

  if (error && !config) {
    return (
      <div className="flex-1 flex items-center justify-center" style={{ backgroundColor: 'var(--codex-bg)' }}>
        <div className="flex flex-col items-center gap-3 max-w-sm text-center">
          <AlertTriangle className="w-6 h-6" style={{ color: '#e5534b' }} />
          <span className="text-[13px]" style={{ color: 'var(--codex-fg)' }}>Failed to load settings</span>
          <span className="text-[12px]" style={{ color: '#666' }}>{error.message}</span>
          <button
            onClick={refetch}
            className="flex items-center gap-2 px-4 py-2 rounded text-[13px] mt-2 transition-colors"
            style={{ backgroundColor: 'var(--codex-accent)', color: 'white' }}
          >
            <RefreshCw className="w-3.5 h-3.5" strokeWidth={1.5} />
            Retry
          </button>
        </div>
      </div>
    );
  }

  return (
    <>
      {/* Main Content Area */}
      <div className="flex-1 flex overflow-hidden">
        {/* Settings Navigation */}
        <nav className="w-[220px] border-r overflow-y-auto" style={{
          backgroundColor: 'var(--codex-bg-secondary)',
          borderColor: 'var(--codex-border-subtle)'
        }}>
          <div className="p-4">
            <h2 className="text-[10px] uppercase tracking-wider mb-3 px-3" style={{
              color: 'var(--codex-fg-subtle)',
              fontWeight: 500
            }}>
              Settings
            </h2>
            <div className="space-y-0.5">
              {sections.map((section) => {
                const isActive = activeSection === section.id;
                return (
                  <button
                    key={section.id}
                    onClick={() => setActiveSection(section.id)}
                    className="w-full flex items-center gap-3 px-3 py-2 rounded-lg text-[13px] transition-all relative"
                    style={{
                      color: isActive ? 'var(--codex-accent)' : 'var(--codex-fg-subtle)',
                      backgroundColor: isActive ? 'var(--codex-accent-dim)' : 'transparent'
                    }}
                    onMouseEnter={(e) => {
                      if (!isActive) e.currentTarget.style.backgroundColor = 'var(--codex-bg)';
                    }}
                    onMouseLeave={(e) => {
                      if (!isActive) e.currentTarget.style.backgroundColor = 'transparent';
                    }}
                  >
                    {isActive && (
                      <div className="absolute left-0 top-1/2 -translate-y-1/2 w-[2px] h-4 rounded-r" style={{
                        backgroundColor: 'var(--codex-accent)'
                      }} />
                    )}
                    <section.icon className="w-4 h-4" strokeWidth={1.5} />
                    {section.label}
                  </button>
                );
              })}
            </div>
          </div>
        </nav>

        {/* Content Area */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Header */}
          <div className="border-b px-8 py-6" style={{
            borderColor: 'var(--codex-border-subtle)',
            backgroundColor: 'var(--codex-bg)'
          }}>
            <h1 className="text-xl mb-1" style={{
              color: 'var(--codex-fg)',
              fontWeight: 400
            }}>
              {sections.find(s => s.id === activeSection)?.label}
            </h1>
            <p className="text-[13px]" style={{ color: '#666' }}>
              {activeSection === 'providers' && 'Configure AI provider connections'}
              {activeSection === 'general' && 'General application settings'}
              {activeSection === 'channels' && 'Configure chat channel integrations'}
              {activeSection === 'agent-defaults' && 'Default agent behavior settings'}
              {activeSection === 'tools' && 'Tool permissions and configuration'}
              {activeSection === 'tasks-todo' && 'Task management preferences'}
              {activeSection === 'calendar' && 'Calendar synchronization settings'}
              {activeSection === 'conversation' && 'Conversation history and memory'}
              {activeSection === 'learning' && 'Agent learning configuration'}
              {activeSection === 'confidence' && 'Confidence threshold settings'}
              {activeSection === 'finance' && 'Financial tracking configuration'}
              {activeSection === 'projects' && 'Project management settings'}
              {activeSection === 'packs-skills' && 'Feature packs and skills'}
              {activeSection === 'plugins' && 'Plugin system configuration'}
            </p>
          </div>

          {/* Content */}
          <div className="flex-1 overflow-y-auto px-8 py-6" style={{ backgroundColor: 'var(--codex-bg)' }}>
            <div className="max-w-3xl">

              {/* GENERAL */}
              {activeSection === 'general' && (
                <>
                  <FormCard title="Timezone" description="Default timezone for the application">
                    <TextInput value={str(config ?? {}, 'timezone', 'UTC')} placeholder="UTC" />
                  </FormCard>
                  <FormCard title="Data Directory" description="Where klyntbot stores its data">
                    <TextInput value={str(config ?? {}, 'dataDir', '~/.klyntbot')} placeholder="~/.klyntbot" />
                  </FormCard>
                  <FormCard title="Gateway Host" description="API gateway host address">
                    <TextInput value={str(gateway, 'host', '0.0.0.0')} placeholder="0.0.0.0" />
                  </FormCard>
                  <FormCard title="Gateway Port" description="API gateway port number">
                    <TextInput value={str(gateway, 'port', '18790')} placeholder="18790" />
                  </FormCard>
                </>
              )}

              {/* PROVIDERS */}
              {activeSection === 'providers' && (
                <>
                  {/* Provider Tabs */}
                  <div className="mb-6 overflow-x-auto">
                    <div className="flex gap-4 pb-2 min-w-max">
                      {providerTabs.map((provider) => {
                        const isActive = provider === activeProvider;
                        return (
                          <button
                            key={provider}
                            onClick={() => setActiveProvider(provider)}
                            className="px-3 py-2 text-[13px] relative transition-colors whitespace-nowrap"
                            style={{
                              color: isActive ? 'var(--codex-accent)' : 'var(--codex-fg-subtle)'
                            }}
                            onMouseEnter={(e) => {
                              if (!isActive) e.currentTarget.style.color = 'var(--codex-fg-muted)';
                            }}
                            onMouseLeave={(e) => {
                              if (!isActive) e.currentTarget.style.color = 'var(--codex-fg-subtle)';
                            }}
                          >
                            {displayName(provider)}
                            {isActive && (
                              <div className="absolute bottom-0 left-0 right-0 h-[2px]" style={{
                                backgroundColor: 'var(--codex-accent)'
                              }} />
                            )}
                          </button>
                        );
                      })}
                    </div>
                  </div>

                  <FormCard>
                    <div className="flex items-start justify-between mb-3">
                      <div>
                        <label className="text-[13px] block mb-1" style={{
                          color: 'var(--codex-fg)',
                          fontWeight: 500
                        }}>
                          API Key
                        </label>
                        <p className="text-[12px]" style={{ color: '#666' }}>
                          Your {displayName(activeProvider)} API key for authentication
                        </p>
                      </div>
                      <button
                        className="px-3 py-1.5 rounded text-[12px] transition-colors"
                        style={{
                          backgroundColor: 'var(--codex-accent)',
                          color: 'white'
                        }}
                        onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)'}
                        onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent)'}
                        // TODO: onClick should PATCH /api/settings/providers with { [activeProvider]: { apiKey: newValue } }
                      >
                        Save
                      </button>
                    </div>
                    <div className="relative">
                      <input
                        type={showApiKey ? 'text' : 'password'}
                        defaultValue={str(currentProviderConfig, 'apiKey', '')}
                        className="w-full px-3 py-2 rounded border outline-none text-[13px] pr-10"
                        style={{
                          backgroundColor: 'var(--codex-bg)',
                          borderColor: 'var(--codex-border)',
                          color: 'var(--codex-fg)',
                          fontFamily: 'var(--font-mono)'
                        }}
                        readOnly
                        // TODO: Remove readOnly and wire onChange to collect new API key value
                      />
                      <button
                        onClick={() => setShowApiKey(!showApiKey)}
                        className="absolute right-2 top-1/2 -translate-y-1/2 p-1"
                        style={{ color: 'var(--codex-fg-subtle)' }}
                      >
                        {showApiKey ? (
                          <EyeOff className="w-4 h-4" strokeWidth={1.5} />
                        ) : (
                          <Eye className="w-4 h-4" strokeWidth={1.5} />
                        )}
                      </button>
                    </div>
                  </FormCard>

                  <FormCard title="Default Model" description="Default model for this provider">
                    <TextInput value={str(currentProviderConfig, 'defaultModel', '')} placeholder="model-name" />
                  </FormCard>

                  <FormCard title="API Base URL" description="Custom API endpoint">
                    <TextInput value={str(currentProviderConfig, 'apiBase', str(currentProviderConfig, 'baseUrl', ''))} placeholder={`https://api.${activeProvider}.com`} />
                  </FormCard>

                  <FormCard title="Extra Headers" description="Additional HTTP headers for API requests">
                    <div className="space-y-2">
                      {(() => {
                        const headers = asRecord(currentProviderConfig.extraHeaders);
                        const entries = Object.entries(headers);
                        if (entries.length === 0) {
                          return (
                            <div className="flex gap-2">
                              <TextInput placeholder="Header name" />
                              <TextInput placeholder="Header value" />
                              <button className="px-3 py-2 rounded border" style={{ borderColor: 'var(--codex-border)', color: 'var(--codex-fg-subtle)' }}>
                                <X className="w-4 h-4" strokeWidth={1.5} />
                              </button>
                            </div>
                          );
                        }
                        return entries.map(([key, val]) => (
                          <div key={key} className="flex gap-2">
                            <TextInput value={key} placeholder="Header name" />
                            <TextInput value={String(val)} placeholder="Header value" />
                            <button className="px-3 py-2 rounded border" style={{ borderColor: 'var(--codex-border)', color: 'var(--codex-fg-subtle)' }}>
                              <X className="w-4 h-4" strokeWidth={1.5} />
                            </button>
                          </div>
                        ));
                      })()}
                      <button className="flex items-center gap-2 px-3 py-2 rounded text-[12px]" style={{ color: 'var(--codex-accent)' }}>
                        <Plus className="w-3.5 h-3.5" strokeWidth={1.5} />
                        Add Header
                      </button>
                    </div>
                  </FormCard>

                  <FormCard>
                    <div className="flex items-start justify-between">
                      <div className="flex-1">
                        <label className="text-[13px] block mb-1" style={{
                          color: 'var(--codex-fg)',
                          fontWeight: 500
                        }}>
                          Native Mode
                        </label>
                        <p className="text-[12px]" style={{ color: '#666' }}>
                          Use provider's native API format
                        </p>
                      </div>
                      {/* TODO: Wire toggle to PATCH /api/settings/providers */}
                      <Toggle checked={bool(currentProviderConfig, 'native', false)} onChange={() => {}} />
                    </div>
                  </FormCard>

                  <FormCard>
                    <div className="flex items-start justify-between mb-3">
                      <div className="flex-1">
                        <label className="text-[13px] block mb-1" style={{
                          color: 'var(--codex-fg)',
                          fontWeight: 500
                        }}>
                          Extended Thinking
                        </label>
                        <p className="text-[12px]" style={{ color: '#666' }}>
                          Allow the model to think longer for complex tasks
                        </p>
                      </div>
                      {/* TODO: Wire toggle to PATCH /api/settings/providers */}
                      <Toggle
                        checked={bool(asRecord(currentProviderConfig.extendedThinking), 'enabled', false)}
                        onChange={() => {}}
                      />
                    </div>
                    {bool(asRecord(currentProviderConfig.extendedThinking), 'enabled', false) && (
                      <div className="space-y-3 pt-3 border-t" style={{ borderColor: 'var(--codex-border)' }}>
                        <div>
                          <label className="text-[12px] block mb-2" style={{ color: 'var(--codex-fg-subtle)' }}>
                            Budget Tokens
                          </label>
                          <TextInput
                            value={str(asRecord(currentProviderConfig.extendedThinking), 'budgetTokens', '')}
                            placeholder="10000"
                          />
                        </div>
                        <div>
                          <label className="text-[12px] block mb-2" style={{ color: 'var(--codex-fg-subtle)' }}>
                            Use For
                          </label>
                          <TextInput
                            value={str(asRecord(currentProviderConfig.extendedThinking), 'useFor', '')}
                            placeholder="Complex reasoning tasks"
                          />
                        </div>
                      </div>
                    )}
                  </FormCard>

                  <FormCard title="API Version">
                    <TextInput value={str(currentProviderConfig, 'apiVersion', '')} placeholder="2023-06-01" />
                  </FormCard>

                  {/* Routing */}
                  <div className="mt-8 mb-4">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                      Routing
                    </h3>
                  </div>

                  <FormCard title="Primary Provider">
                    <select className="w-full px-3 py-2 rounded border text-[13px] appearance-none" style={{
                      backgroundColor: 'var(--codex-bg)',
                      borderColor: 'var(--codex-border)',
                      color: 'var(--codex-fg)'
                    }}
                      // TODO: Wire onChange to PATCH /api/settings/providers
                      value={str(asRecord(config?.routing), 'primary', providerTabs[0] ?? '')}
                      onChange={() => {}} // TODO: PATCH on change
                    >
                      {providerTabs.map((p) => (
                        <option key={p} value={p}>{displayName(p)}</option>
                      ))}
                    </select>
                  </FormCard>

                  <FormCard title="Fallback Provider">
                    <select className="w-full px-3 py-2 rounded border text-[13px]" style={{
                      backgroundColor: 'var(--codex-bg)',
                      borderColor: 'var(--codex-border)',
                      color: 'var(--codex-fg)'
                    }}
                      // TODO: Wire onChange to PATCH /api/settings/providers
                      value={str(asRecord(config?.routing), 'fallback', '')}
                      onChange={() => {}} // TODO: PATCH on change
                    >
                      {providerTabs.map((p) => (
                        <option key={p} value={p}>{displayName(p)}</option>
                      ))}
                    </select>
                  </FormCard>

                  <FormCard title="Classifier Model">
                    <TextInput
                      value={str(asRecord(config?.routing), 'classifierModel', '')}
                      placeholder="gpt-4o-mini"
                    />
                  </FormCard>
                </>
              )}

              {/* CHANNELS */}
              {activeSection === 'channels' && (
                <>
                  <div className="mb-6 overflow-x-auto">
                    <div className="flex gap-4 pb-2 min-w-max">
                      {channelTabs.map((channel) => {
                        const isActive = channel === activeChannel;
                        return (
                          <button
                            key={channel}
                            onClick={() => setActiveChannel(channel)}
                            className="px-3 py-2 text-[13px] relative transition-colors whitespace-nowrap"
                            style={{
                              color: isActive ? 'var(--codex-accent)' : 'var(--codex-fg-subtle)'
                            }}
                          >
                            {displayName(channel)}
                            {isActive && (
                              <div className="absolute bottom-0 left-0 right-0 h-[2px]" style={{
                                backgroundColor: 'var(--codex-accent)'
                              }} />
                            )}
                          </button>
                        );
                      })}
                    </div>
                  </div>

                  <FormCard>
                    <div className="flex items-center justify-between mb-4">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                        Enabled
                      </label>
                      {/* TODO: Wire toggle to PATCH /api/settings/channels */}
                      <Toggle checked={bool(currentChannelConfig, 'enabled', false)} onChange={() => {}} />
                    </div>
                  </FormCard>

                  <FormCard title="Token" description="Bot authentication token">
                    <TextInput
                      value={str(currentChannelConfig, 'token', str(currentChannelConfig, 'botToken', ''))}
                      secret
                    />
                  </FormCard>

                  <FormCard title="Allow From" description="Allowed user IDs (comma-separated)">
                    <TextInput
                      value={(() => {
                        const ids = arr(currentChannelConfig, 'allowedChatIds');
                        return ids.length > 0 ? ids.join(', ') : '';
                      })()}
                      placeholder="user1, user2, user3"
                    />
                  </FormCard>

                  <FormCard title="Proxy" description="Proxy URL for connections">
                    <TextInput value={str(currentChannelConfig, 'proxy', '')} placeholder="socks5://127.0.0.1:1080" />
                  </FormCard>
                </>
              )}

              {/* AGENT DEFAULTS */}
              {activeSection === 'agent-defaults' && (
                <>
                  <FormCard title="Model" description="Default model identifier">
                    <TextInput value={str(agentDefaults, 'model', '')} />
                  </FormCard>
                  <FormCard title="Workspace" description="Agent workspace directory">
                    <TextInput value={str(agentDefaults, 'workspace', str(agentDefaults, 'workspacePath', ''))} placeholder="~/.klyntbot/workspace" />
                  </FormCard>
                  <FormCard title="Provider" description="Override default provider (optional)">
                    <TextInput value={str(agentDefaults, 'provider', '')} placeholder="anthropic" />
                  </FormCard>
                  <FormCard title="Max Tokens">
                    <TextInput value={str(agentDefaults, 'maxTokens', '')} />
                  </FormCard>
                  <FormCard title="Temperature" description="Model creativity (0-1)">
                    <input
                      type="range"
                      min="0"
                      max="1"
                      step="0.1"
                      value={num(agentDefaults, 'temperature', 0.7)}
                      className="w-full"
                      readOnly
                      // TODO: Wire onChange to PATCH /api/settings/agents
                    />
                    <div className="flex justify-between text-[12px] mt-2" style={{ color: 'var(--codex-fg-subtle)' }}>
                      <span>0</span>
                      <span style={{ color: 'var(--codex-accent)' }}>{num(agentDefaults, 'temperature', 0.7)}</span>
                      <span>1</span>
                    </div>
                  </FormCard>
                  <FormCard title="Max Tool Iterations">
                    <TextInput value={str(agentDefaults, 'maxToolIterations', str(agentDefaults, 'maxIterations', ''))} />
                  </FormCard>
                  <FormCard title="Permission Mode">
                    <TextInput value={str(agentDefaults, 'permissionMode', '')} />
                  </FormCard>
                  <FormCard title="System Prompt File">
                    <TextInput value={str(agentDefaults, 'systemPromptFile', '')} placeholder="(none)" />
                  </FormCard>
                </>
              )}

              {/* TOOLS */}
              {activeSection === 'tools' && (
                <>
                  <div className="mb-4">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Web</h3>
                  </div>
                  <FormCard title="Brave API Key">
                    <TextInput secret value={str(toolsWeb, 'braveApiKey', str(toolsWeb, 'apiKey', ''))} />
                  </FormCard>
                  <FormCard title="Max Results">
                    <TextInput value={str(toolsWeb, 'maxResults', str(tools, 'maxSearchResults', ''))} />
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Browser</h3>
                  </div>
                  <FormCard>
                    <div className="flex items-center justify-between mb-4">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enabled</label>
                      {/* TODO: Wire toggle to PATCH /api/settings/tools */}
                      <Toggle checked={bool(toolsBrowser, 'enabled', false)} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Trust Level">
                    <select className="w-full px-3 py-2 rounded border text-[13px]" style={{
                      backgroundColor: 'var(--codex-bg)',
                      borderColor: 'var(--codex-border)',
                      color: 'var(--codex-fg)'
                    }}
                      value={str(toolsBrowser, 'trustLevel', 'strict')}
                      onChange={() => {}} // TODO: Wire to PATCH /api/settings/tools
                    >
                      <option>strict</option>
                      <option>autonomous</option>
                      <option>full</option>
                    </select>
                  </FormCard>
                  <FormCard title="Session Timeout (seconds)">
                    <TextInput value={str(toolsBrowser, 'sessionTimeout', '')} />
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Permissions</h3>
                  </div>
                  <FormCard>
                    <div className="flex items-center justify-between">
                      <div className="flex-1">
                        <label className="text-[13px] block mb-1" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                          Restrict to Workspace
                        </label>
                        <p className="text-[12px]" style={{ color: '#666' }}>
                          Limit file operations to workspace directory
                        </p>
                      </div>
                      {/* TODO: Wire toggle to PATCH /api/settings/tools */}
                      <Toggle checked={bool(tools, 'restrictToWorkspace', false)} onChange={() => {}} />
                    </div>
                  </FormCard>

                  {str(tools, 'workspacePath') && (
                    <FormCard title="Workspace Path">
                      <TextInput value={str(tools, 'workspacePath')} />
                    </FormCard>
                  )}

                  {arr(tools, 'disabled').length > 0 && (
                    <FormCard title="Disabled Tools">
                      <TextInput value={arr(tools, 'disabled').join(', ')} />
                    </FormCard>
                  )}

                  {str(tools, 'maxFileSizeBytes') && (
                    <FormCard title="Max File Size (bytes)">
                      <TextInput value={str(tools, 'maxFileSizeBytes')} />
                    </FormCard>
                  )}
                </>
              )}

              {/* TASKS & TODO */}
              {activeSection === 'tasks-todo' && (
                <>
                  <FormCard title="Creation Mode">
                    <select className="w-full px-3 py-2 rounded border text-[13px]" style={{
                      backgroundColor: 'var(--codex-bg)',
                      borderColor: 'var(--codex-border)',
                      color: 'var(--codex-fg)'
                    }}
                      value={str(todo, 'creationMode', 'ask-first')}
                      onChange={() => {}} // TODO: Wire to PATCH /api/settings/todo
                    >
                      <option>ask-first</option>
                      <option>yolo</option>
                      <option>party</option>
                    </select>
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enrichment</h3>
                  </div>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enabled</label>
                      {/* TODO: Wire toggle to PATCH /api/settings/todo */}
                      <Toggle checked={bool(enrichment, 'enabled', false)} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Auto Apply Threshold">
                    <TextInput value={str(enrichment, 'autoApplyThreshold', '')} />
                  </FormCard>
                  <FormCard>
                    <div className="flex items-center justify-between">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Use LLM</label>
                      {/* TODO: Wire toggle to PATCH /api/settings/todo */}
                      <Toggle checked={bool(enrichment, 'useLlm', false)} onChange={() => {}} />
                    </div>
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Search</h3>
                  </div>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enabled</label>
                      {/* TODO: Wire toggle to PATCH /api/settings/todo */}
                      <Toggle checked={bool(search, 'enabled', false)} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Semantic Threshold">
                    <TextInput value={str(search, 'semanticThreshold', '')} />
                  </FormCard>
                  <FormCard title="Embedding Model">
                    <TextInput value={str(search, 'embeddingModel', '')} />
                  </FormCard>
                  <FormCard title="RRF K">
                    <TextInput value={str(search, 'rrfK', '')} />
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Notifications</h3>
                  </div>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Focus Reminders</label>
                      {/* TODO: Wire toggle to PATCH /api/settings/todo */}
                      <Toggle checked={bool(todoNotifications, 'focusReminders', false)} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Daily Digest</label>
                      {/* TODO: Wire toggle to PATCH /api/settings/todo */}
                      <Toggle checked={bool(todoNotifications, 'dailyDigest', false)} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Digest Time">
                    <TextInput value={str(todoNotifications, 'digestTime', '')} />
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Focus</h3>
                  </div>
                  <FormCard title="Max Slots">
                    <TextInput value={str(todoFocus, 'maxSlots', '')} />
                  </FormCard>
                  <FormCard title="Deadline Hours">
                    <TextInput value={str(todoFocus, 'deadlineHours', '')} />
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Daily Planning</h3>
                  </div>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enabled</label>
                      {/* TODO: Wire toggle to PATCH /api/settings/todo */}
                      <Toggle checked={bool(todoDailyPlanning, 'enabled', false)} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Planning Time">
                    <TextInput value={str(todoDailyPlanning, 'time', '')} />
                  </FormCard>
                </>
              )}

              {/* CALENDAR */}
              {activeSection === 'calendar' && (
                <>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <div className="flex-1">
                        <label className="text-[13px] block mb-1" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                          Bidirectional Sync
                        </label>
                        <p className="text-[12px]" style={{ color: '#666' }}>
                          Sync changes both ways
                        </p>
                      </div>
                      {/* TODO: Wire toggle to PATCH /api/settings/calendar */}
                      <Toggle checked={bool(calendar, 'bidirectionalSync', bool(calendar, 'bidirectional', false))} onChange={() => {}} />
                    </div>
                  </FormCard>

                  <FormCard title="Conflict Resolution">
                    <select className="w-full px-3 py-2 rounded border text-[13px]" style={{
                      backgroundColor: 'var(--codex-bg)',
                      borderColor: 'var(--codex-border)',
                      color: 'var(--codex-fg)'
                    }}
                      value={str(calendar, 'conflictResolution', 'manual')}
                      onChange={() => {}} // TODO: Wire to PATCH /api/settings/calendar
                    >
                      <option>manual</option>
                      <option>local-wins</option>
                      <option>remote-wins</option>
                      <option>newest-wins</option>
                    </select>
                  </FormCard>

                  <div className="flex justify-between items-center my-6">
                    <h3 className="text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Providers</h3>
                    <button className="flex items-center gap-2 px-3 py-1.5 rounded text-[12px]" style={{
                      backgroundColor: 'var(--codex-accent)',
                      color: 'white'
                    }}
                      // TODO: Wire onClick to add a new calendar provider via PATCH
                    >
                      <Plus className="w-3.5 h-3.5" strokeWidth={1.5} />
                      Add Provider
                    </button>
                  </div>

                  {calendarProviders.length > 0 ? calendarProviders.map((prov, idx) => {
                    const p = asRecord(prov);
                    return (
                      <div key={idx} className="p-4 rounded-lg mb-4" style={{
                        backgroundColor: '#141414',
                        border: '1px solid var(--codex-border)'
                      }}>
                        <div className="flex items-center justify-between mb-4">
                          <div>
                            <div className="text-[13px] mb-1" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                              {str(p, 'name', str(p, 'calendarName', `Provider ${idx + 1}`))}
                            </div>
                            <div className="text-[11px]" style={{ color: '#666' }}>
                              {str(p, 'provider', str(p, 'type', 'CalDAV'))}
                            </div>
                          </div>
                          {/* TODO: Wire toggle to PATCH /api/settings/calendar */}
                          <Toggle checked={bool(p, 'enabled', true)} onChange={() => {}} />
                        </div>
                        <div className="space-y-3">
                          <div>
                            <label className="text-[11px] block mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>CalDAV URL</label>
                            <TextInput value={str(p, 'url', str(p, 'caldavUrl', ''))} placeholder="https://calendar.example.com/..." />
                          </div>
                          <div className="grid grid-cols-2 gap-3">
                            <div>
                              <label className="text-[11px] block mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>Username</label>
                              <TextInput value={str(p, 'username', '')} placeholder="user@example.com" />
                            </div>
                            <div>
                              <label className="text-[11px] block mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>Password</label>
                              <TextInput value={str(p, 'password', '')} secret />
                            </div>
                          </div>
                          <div>
                            <label className="text-[11px] block mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>Calendar Name</label>
                            <TextInput value={str(p, 'calendarName', '')} />
                          </div>
                          <div className="grid grid-cols-2 gap-3">
                            <div>
                              <label className="text-[11px] block mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>Sync Interval (min)</label>
                              <TextInput value={str(p, 'syncIntervalMinutes', str(p, 'syncInterval', ''))} />
                            </div>
                            <div className="flex items-end">
                              <label className="flex items-center gap-2 text-[11px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                                <input type="checkbox" checked={bool(p, 'syncDueDates', bool(p, 'autoSyncDueDates', false))} readOnly />
                                Auto sync due dates
                              </label>
                            </div>
                          </div>
                        </div>
                      </div>
                    );
                  }) : (
                    // Fallback when no providers are configured — show empty card
                    <div className="p-4 rounded-lg mb-4" style={{
                      backgroundColor: '#141414',
                      border: '1px solid var(--codex-border)'
                    }}>
                      <div className="text-[13px] text-center py-4" style={{ color: '#666' }}>
                        No calendar providers configured
                      </div>
                    </div>
                  )}
                </>
              )}

              {/* CONVERSATION */}
              {activeSection === 'conversation' && (
                <>
                  <div className="mb-4">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Embedding</h3>
                  </div>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enabled</label>
                      {/* TODO: Wire toggle to PATCH /api/settings/conversation */}
                      <Toggle checked={bool(conversationEmbedding, 'enabled', false)} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Exclude Channels">
                    <TextInput
                      value={arr(conversationEmbedding, 'excludeChannels').join(', ')}
                      placeholder="channel1, channel2"
                    />
                  </FormCard>
                  <FormCard title="Exclude Roles">
                    <TextInput
                      value={arr(conversationEmbedding, 'excludeRoles').join(', ')}
                      placeholder="role1, role2"
                    />
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Search</h3>
                  </div>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enabled</label>
                      {/* TODO: Wire toggle to PATCH /api/settings/conversation */}
                      <Toggle checked={bool(conversationSearch, 'enabled', false)} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Semantic Threshold">
                    <TextInput value={str(conversationSearch, 'semanticThreshold', '')} />
                  </FormCard>
                  <FormCard title="Max Results">
                    <TextInput value={str(conversationSearch, 'maxResults', '')} />
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Session</h3>
                  </div>
                  <FormCard title="History Limit">
                    <TextInput value={str(conversation, 'maxHistory', str(conversationSession, 'historyLimit', ''))} />
                  </FormCard>
                  <FormCard title="TTL (days)">
                    <TextInput value={str(conversationSession, 'ttlDays', str(conversationSession, 'ttl', ''))} />
                  </FormCard>
                  <FormCard title="Cleanup Interval (hours)">
                    <TextInput value={str(conversationSession, 'cleanupIntervalHours', str(conversationSession, 'cleanupInterval', ''))} />
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Memory</h3>
                  </div>
                  <FormCard title="Decay Half Life (days)">
                    <TextInput value={str(conversationMemory, 'decayHalfLifeDays', str(conversationMemory, 'decayHalfLife', ''))} />
                  </FormCard>
                  <FormCard title="Max Age (days)">
                    <TextInput value={str(conversationMemory, 'maxAgeDays', str(conversationMemory, 'maxAge', ''))} />
                  </FormCard>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Consolidation Enabled</label>
                      {/* TODO: Wire toggle to PATCH /api/settings/conversation */}
                      <Toggle checked={bool(conversationMemory, 'consolidationEnabled', bool(conversationMemory, 'consolidation', false))} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Maintenance Interval (hours)">
                    <TextInput value={str(conversationMemory, 'maintenanceIntervalHours', str(conversationMemory, 'maintenanceInterval', ''))} />
                  </FormCard>
                </>
              )}

              {/* LEARNING */}
              {activeSection === 'learning' && (
                <>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enabled</label>
                      {/* TODO: Wire toggle to PATCH /api/settings/learning */}
                      <Toggle checked={bool(learning, 'enabled', false)} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Analysis Interval (seconds)">
                    <TextInput value={str(learning, 'analysisInterval', str(learning, 'analysisIntervalSecs', ''))} />
                  </FormCard>
                  <FormCard title="Min Threshold">
                    <TextInput value={str(learning, 'minThreshold', '')} />
                  </FormCard>
                  <FormCard title="Max Threshold">
                    <TextInput value={str(learning, 'maxThreshold', '')} />
                  </FormCard>
                  <FormCard title="Min Outcomes for Adaptation">
                    <TextInput value={str(learning, 'minOutcomesForAdaptation', str(learning, 'minOutcomes', ''))} />
                  </FormCard>
                </>
              )}

              {/* CONFIDENCE */}
              {activeSection === 'confidence' && (
                <>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enabled</label>
                      {/* TODO: Wire toggle to PATCH /api/settings/confidence */}
                      <Toggle checked={bool(confidence, 'enabled', false)} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Threshold" description="Minimum confidence level (0-1)">
                    <input
                      type="range"
                      min="0"
                      max="1"
                      step="0.1"
                      value={num(confidence, 'threshold', 0.7)}
                      className="w-full"
                      readOnly
                      // TODO: Wire onChange to PATCH /api/settings/confidence
                    />
                    <div className="flex justify-between text-[12px] mt-2" style={{ color: 'var(--codex-fg-subtle)' }}>
                      <span>0</span>
                      <span style={{ color: 'var(--codex-accent)' }}>{num(confidence, 'threshold', 0.7)}</span>
                      <span>1</span>
                    </div>
                  </FormCard>
                  <FormCard title="Tool Overrides" description="Per-tool confidence settings">
                    <div className="space-y-2">
                      {(() => {
                        const overrides = asRecord(confidence.toolOverrides);
                        const entries = Object.entries(overrides);
                        if (entries.length === 0) {
                          return (
                            <div className="flex gap-2">
                              <TextInput placeholder="Tool name" />
                              <TextInput placeholder="0.8" />
                              <button className="px-3 py-2 rounded border" style={{ borderColor: 'var(--codex-border)', color: 'var(--codex-fg-subtle)' }}>
                                <X className="w-4 h-4" strokeWidth={1.5} />
                              </button>
                            </div>
                          );
                        }
                        return entries.map(([tool, threshold]) => (
                          <div key={tool} className="flex gap-2">
                            <TextInput value={tool} placeholder="Tool name" />
                            <TextInput value={String(threshold)} placeholder="0.8" />
                            <button className="px-3 py-2 rounded border" style={{ borderColor: 'var(--codex-border)', color: 'var(--codex-fg-subtle)' }}>
                              <X className="w-4 h-4" strokeWidth={1.5} />
                            </button>
                          </div>
                        ));
                      })()}
                      <button className="flex items-center gap-2 px-3 py-2 rounded text-[12px]" style={{ color: 'var(--codex-accent)' }}>
                        <Plus className="w-3.5 h-3.5" strokeWidth={1.5} />
                        Add Override
                      </button>
                    </div>
                  </FormCard>
                </>
              )}

              {/* FINANCE */}
              {activeSection === 'finance' && (() => {
                const selectCss: React.CSSProperties = { backgroundColor: 'var(--codex-bg)', borderColor: 'var(--codex-border)', color: 'var(--codex-fg)' };
                const inputCss: React.CSSProperties = { ...selectCss, fontFamily: 'var(--font-ui)' };
                const currencies = ['USD', 'VND', 'EUR', 'GBP', 'JPY', 'KRW', 'CNY', 'THB', 'BTC', 'ETH', 'USDT'];
                const currSymbols: Record<string, string> = { USD: '$', EUR: '€', GBP: '£', JPY: '¥', KRW: '₩', VND: '₫', CNY: '¥', THB: '฿', BTC: '₿' };

                // Key exchange rates to display relative to default currency
                const defaultCur = str(finance, 'defaultCurrency', 'USD');
                const rateDisplayCurrencies = ['USD', 'VND', 'EUR', 'GBP', 'JPY', 'BTC', 'ETH', 'USDT'].filter(c => c !== defaultCur);

                const formatRate = (from: string, to: string) => {
                  if (!liveRates) return '—';
                  const fromR = liveRates[from] ?? 1;
                  const toR = liveRates[to] ?? 1;
                  const rate = toR / fromR; // how many "to" units per 1 "from" unit... actually inverted
                  // We want: 1 FROM = X TO. rates are "value of 1 unit in USD"
                  // 1 FROM in USD = fromR. 1 TO in USD = toR. So 1 FROM = fromR/toR TO units.
                  const r = fromR / toR;
                  if (r >= 100) return r.toLocaleString('en-US', { maximumFractionDigits: 0 });
                  if (r >= 1) return r.toLocaleString('en-US', { maximumFractionDigits: 2 });
                  if (r >= 0.01) return r.toLocaleString('en-US', { maximumFractionDigits: 4 });
                  return r.toLocaleString('en-US', { maximumFractionDigits: 8 });
                };

                return (
                <>
                  {/* Save status indicator */}
                  {saveStatus && (
                    <div className="mb-4 px-4 py-2.5 rounded-lg text-[13px]" style={{
                      backgroundColor: saveStatus.ok ? 'rgba(16,163,127,0.1)' : 'rgba(239,68,68,0.1)',
                      color: saveStatus.ok ? '#10a37f' : '#ef4444',
                      border: `1px solid ${saveStatus.ok ? 'rgba(16,163,127,0.3)' : 'rgba(239,68,68,0.3)'}`
                    }}>
                      {saveStatus.msg}
                    </div>
                  )}

                  {/* ── General ──────────────────────────────────────────────── */}
                  <FormCard>
                    <div className="flex items-center justify-between">
                      <div>
                        <label className="text-[13px] block mb-0.5" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Finance Module</label>
                        <p className="text-[12px]" style={{ color: '#666' }}>Enable financial tracking, budgets, and investment monitoring</p>
                      </div>
                      <Toggle checked={bool(finance, 'enabled', false)} onChange={() => patchSection('finance', { enabled: !bool(finance, 'enabled', false) })} />
                    </div>
                  </FormCard>

                  <FormCard title="Display Currency" description="All amounts on the Finance page will be converted to this currency using live exchange rates">
                    <select className="w-full px-3 py-2 rounded border text-[13px]" style={selectCss}
                      value={str(finance, 'defaultCurrency', 'USD')}
                      onChange={(e) => patchSection('finance', { defaultCurrency: e.target.value })}
                    >
                      {currencies.map(c => <option key={c} value={c}>{c}{currSymbols[c] ? ` (${currSymbols[c]})` : ''}</option>)}
                    </select>
                  </FormCard>

                  <FormCard title="Proactivity Level" description="How proactively the AI agent manages your finances">
                    <select className="w-full px-3 py-2 rounded border text-[13px]" style={selectCss}
                      value={str(finance, 'proactivityLevel', 'full')}
                      onChange={(e) => patchSection('finance', { proactivityLevel: e.target.value })}
                    >
                      <option value="full">Full — auto-categorize, budget alerts, spending insights</option>
                      <option value="moderate">Moderate — alerts and categorization only</option>
                      <option value="reactive">Reactive — only respond when asked</option>
                    </select>
                  </FormCard>

                  {/* ── Exchange Rates ───────────────────────────────────────── */}
                  <div className="mb-4 mt-8">
                    <div className="flex items-center justify-between">
                      <h3 className="text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Exchange Rates</h3>
                      <button
                        onClick={() => fetchRates(true)}
                        disabled={ratesLoadingState}
                        className="flex items-center gap-1.5 px-3 py-1.5 rounded text-[12px] transition-colors"
                        style={{ backgroundColor: '#1a1a1a', color: 'var(--codex-fg-subtle)', border: '1px solid var(--codex-border)', opacity: ratesLoadingState ? 0.5 : 1 }}
                        onMouseEnter={(e) => { if (!ratesLoadingState) e.currentTarget.style.backgroundColor = '#222'; }}
                        onMouseLeave={(e) => e.currentTarget.style.backgroundColor = '#1a1a1a'}
                      >
                        <RefreshCw className={`w-3 h-3 ${ratesLoadingState ? 'animate-spin' : ''}`} strokeWidth={1.5} />
                        {ratesLoadingState ? 'Fetching...' : 'Refresh'}
                      </button>
                    </div>
                    {ratesTimestamp && (
                      <p className="text-[11px] mt-1" style={{ color: '#555' }}>Last updated: {ratesTimestamp} · Source: open.er-api.com + CoinGecko</p>
                    )}
                  </div>

                  <div className="p-4 rounded-lg mb-4" style={{ backgroundColor: '#141414', border: '1px solid var(--codex-border)' }}>
                    {!liveRates ? (
                      <div className="flex items-center gap-2 py-2">
                        <Loader2 className="w-4 h-4 animate-spin" style={{ color: 'var(--codex-accent)' }} strokeWidth={1.5} />
                        <span className="text-[13px]" style={{ color: 'var(--codex-fg-subtle)' }}>Loading exchange rates...</span>
                      </div>
                    ) : (
                      <div className="grid grid-cols-2 gap-x-6 gap-y-2">
                        {rateDisplayCurrencies.map(cur => {
                          const rate = liveRates[cur];
                          if (!rate) return null;
                          return (
                            <div key={cur} className="flex items-center justify-between py-1.5">
                              <span className="text-[13px]" style={{ color: 'var(--codex-fg-muted)' }}>
                                {currSymbols[cur] ? `${cur} (${currSymbols[cur]})` : cur}
                              </span>
                              <span className="text-[13px] tabular-nums" style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)' }}>
                                {formatRate(cur, defaultCur)}
                              </span>
                            </div>
                          );
                        })}
                      </div>
                    )}
                    <p className="text-[11px] mt-3 pt-3" style={{ color: '#555', borderTop: '1px solid var(--codex-border)' }}>
                      Rates shown as 1 {defaultCur} = X units. Cached for 1 hour. Finance page auto-converts all amounts.
                    </p>
                  </div>

                  {/* ── Inflation ────────────────────────────────────────────── */}
                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Inflation</h3>
                    <p className="text-[12px] mt-0.5" style={{ color: '#666' }}>Used for real-return calculations on goals and investments</p>
                  </div>
                  <div className="grid grid-cols-2 gap-3">
                    <FormCard title="Annual Rate (%)">
                      <input type="number" step="0.1" className="w-full px-3 py-2 rounded border outline-none text-[13px]" style={inputCss}
                        defaultValue={num(financeInflation, 'rate', 3.3)}
                        onChange={(e) => debouncedPatch('finance', { inflation: { rate: parseFloat(e.target.value) || 0 } })}
                      />
                    </FormCard>
                    <FormCard title="Source">
                      <select className="w-full px-3 py-2 rounded border text-[13px]" style={selectCss}
                        value={str(financeInflation, 'source', 'manual')}
                        onChange={(e) => patchSection('finance', { inflation: { source: e.target.value } })}
                      >
                        <option value="manual">Manual</option>
                        <option value="api">API (auto-fetch)</option>
                      </select>
                    </FormCard>
                  </div>

                  {/* ── Expected Returns ─────────────────────────────────────── */}
                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Expected Annual Returns (%)</h3>
                    <p className="text-[12px] mt-0.5" style={{ color: '#666' }}>Default return assumptions for goal projections</p>
                  </div>
                  <div className="grid grid-cols-2 gap-3">
                    {[
                      { key: 'stocks', label: 'Stocks', def: 10 },
                      { key: 'crypto', label: 'Crypto', def: 15 },
                      { key: 'realEstate', label: 'Real Estate', def: 8 },
                      { key: 'bonds', label: 'Bonds', def: 5 },
                    ].map(({ key, label, def }) => (
                      <FormCard key={key} title={label}>
                        <input type="number" step="0.5" className="w-full px-3 py-2 rounded border outline-none text-[13px]" style={inputCss}
                          defaultValue={num(financeExpectedReturns, key, def)}
                          onChange={(e) => debouncedPatch('finance', { expectedReturns: { [key]: parseFloat(e.target.value) || 0 } })}
                        />
                      </FormCard>
                    ))}
                  </div>

                  {/* ── Budgeting ────────────────────────────────────────────── */}
                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Budgeting</h3>
                  </div>
                  <div className="grid grid-cols-2 gap-3">
                    <FormCard title="Default Method">
                      <select className="w-full px-3 py-2 rounded border text-[13px]" style={selectCss}
                        value={str(financeBudgeting, 'defaultMethod', 'standard')}
                        onChange={(e) => patchSection('finance', { budgeting: { defaultMethod: e.target.value } })}
                      >
                        <option value="standard">Standard (envelope)</option>
                        <option value="six_jar">Six Jar Method</option>
                      </select>
                    </FormCard>
                    <FormCard title="Alert Threshold (%)" description="Warn when spending exceeds this % of budget">
                      <input type="number" min="0" max="100" className="w-full px-3 py-2 rounded border outline-none text-[13px]" style={inputCss}
                        defaultValue={num(financeBudgeting, 'alertThreshold', 80)}
                        onChange={(e) => debouncedPatch('finance', { budgeting: { alertThreshold: parseInt(e.target.value) || 80 } })}
                      />
                    </FormCard>
                  </div>

                  <FormCard title="Six Jar Ratios" description="Income allocation percentages (should sum to 100%)">
                    <div className="grid grid-cols-3 gap-3 mt-1">
                      {[
                        { key: 'essentials', label: 'Essentials', def: 55 },
                        { key: 'savings', label: 'Savings', def: 10 },
                        { key: 'investment', label: 'Investment', def: 10 },
                        { key: 'education', label: 'Education', def: 10 },
                        { key: 'entertainment', label: 'Play', def: 10 },
                        { key: 'charity', label: 'Charity', def: 5 },
                      ].map(({ key, label, def }) => (
                        <div key={key}>
                          <label className="text-[11px] block mb-1" style={{ color: '#666' }}>{label}</label>
                          <div className="relative">
                            <input type="number" min="0" max="100" className="w-full px-3 py-1.5 pr-7 rounded border outline-none text-[13px]" style={inputCss}
                              defaultValue={num(sixJarRatios, key, def)}
                              onChange={(e) => debouncedPatch('finance', { budgeting: { sixJarRatios: { [key]: parseInt(e.target.value) || 0 } } })}
                            />
                            <span className="absolute right-2.5 top-1/2 -translate-y-1/2 text-[12px]" style={{ color: '#555' }}>%</span>
                          </div>
                        </div>
                      ))}
                    </div>
                  </FormCard>

                  {/* ── Categories ────────────────────────────────────────────── */}
                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Auto-Categorization</h3>
                    <p className="text-[12px] mt-0.5" style={{ color: '#666' }}>AI-powered automatic transaction categorization</p>
                  </div>
                  <FormCard>
                    <div className="flex items-center justify-between mb-4">
                      <div>
                        <label className="text-[13px] block mb-0.5" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Auto-Categorize Transactions</label>
                        <p className="text-[12px]" style={{ color: '#666' }}>Automatically assign categories to new transactions</p>
                      </div>
                      <Toggle checked={bool(financeCategories, 'autoCategorize', true)} onChange={() => patchSection('finance', { categories: { autoCategorize: !bool(financeCategories, 'autoCategorize', true) } })} />
                    </div>
                    <div>
                      <label className="text-[12px] block mb-1.5" style={{ color: '#888' }}>Confidence Threshold</label>
                      <div className="flex items-center gap-3">
                        <input type="range" min="0" max="100" step="5" className="flex-1"
                          value={Math.round(num(financeCategories, 'confidenceThreshold', 0.8) * 100)}
                          onChange={(e) => patchSection('finance', { categories: { confidenceThreshold: parseInt(e.target.value) / 100 } })}
                        />
                        <span className="text-[13px] tabular-nums w-10 text-right" style={{ color: 'var(--codex-fg)', fontFamily: 'var(--font-mono)' }}>
                          {Math.round(num(financeCategories, 'confidenceThreshold', 0.8) * 100)}%
                        </span>
                      </div>
                      <p className="text-[11px] mt-1" style={{ color: '#555' }}>Only auto-apply when AI confidence exceeds this threshold</p>
                    </div>
                  </FormCard>

                  {/* ── Price Refresh ──────────────────────────────────────── */}
                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Price Refresh</h3>
                    <p className="text-[12px] mt-0.5" style={{ color: '#666' }}>Automatic investment price updates</p>
                  </div>
                  <FormCard>
                    <div className="flex items-center justify-between mb-4">
                      <div>
                        <label className="text-[13px] block mb-0.5" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Auto-Refresh Prices</label>
                        <p className="text-[12px]" style={{ color: '#666' }}>Periodically fetch current prices for investments</p>
                      </div>
                      <Toggle checked={bool(financePriceRefresh, 'enabled', true)} onChange={() => patchSection('finance', { priceRefresh: { enabled: !bool(financePriceRefresh, 'enabled', true) } })} />
                    </div>
                    <div className="grid grid-cols-2 gap-3">
                      <div>
                        <label className="text-[12px] block mb-1.5" style={{ color: '#888' }}>Refresh Interval</label>
                        <div className="relative">
                          <input type="number" min="1" max="24" className="w-full px-3 py-2 pr-14 rounded border outline-none text-[13px]" style={inputCss}
                            defaultValue={num(financePriceRefresh, 'intervalHours', 4)}
                            onChange={(e) => debouncedPatch('finance', { priceRefresh: { intervalHours: parseInt(e.target.value) || 4 } })}
                          />
                          <span className="absolute right-3 top-1/2 -translate-y-1/2 text-[12px]" style={{ color: '#555' }}>hours</span>
                        </div>
                      </div>
                      <div>
                        <label className="text-[12px] block mb-1.5" style={{ color: '#888' }}>Cache TTL</label>
                        <div className="relative">
                          <input type="number" min="1" max="60" className="w-full px-3 py-2 pr-10 rounded border outline-none text-[13px]" style={inputCss}
                            defaultValue={num(financePriceRefresh, 'cacheTtlMinutes', 15)}
                            onChange={(e) => debouncedPatch('finance', { priceRefresh: { cacheTtlMinutes: parseInt(e.target.value) || 15 } })}
                          />
                          <span className="absolute right-3 top-1/2 -translate-y-1/2 text-[12px]" style={{ color: '#555' }}>min</span>
                        </div>
                      </div>
                    </div>
                  </FormCard>

                  {/* ── Scheduling ─────────────────────────────────────────── */}
                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Scheduling</h3>
                    <p className="text-[12px] mt-0.5" style={{ color: '#666' }}>Automated financial review and reporting schedule</p>
                  </div>
                  <div className="grid grid-cols-2 gap-3">
                    <FormCard title="Daily Review Time">
                      <input type="time" className="w-full px-3 py-2 rounded border outline-none text-[13px]" style={inputCss}
                        defaultValue={str(financeScheduling, 'dailyReviewTime', '21:00')}
                        onChange={(e) => debouncedPatch('finance', { scheduling: { dailyReviewTime: e.target.value } })}
                      />
                    </FormCard>
                    <FormCard title="Budget Check Time">
                      <input type="time" className="w-full px-3 py-2 rounded border outline-none text-[13px]" style={inputCss}
                        defaultValue={str(financeScheduling, 'budgetCheckTime', '09:00')}
                        onChange={(e) => debouncedPatch('finance', { scheduling: { budgetCheckTime: e.target.value } })}
                      />
                    </FormCard>
                  </div>
                  <FormCard title="Weekly Report Day">
                    <select className="w-full px-3 py-2 rounded border text-[13px]" style={selectCss}
                      value={str(financeScheduling, 'weeklyReportDay', 'monday')}
                      onChange={(e) => patchSection('finance', { scheduling: { weeklyReportDay: e.target.value } })}
                    >
                      {['monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday', 'sunday'].map(d => (
                        <option key={d} value={d}>{d.charAt(0).toUpperCase() + d.slice(1)}</option>
                      ))}
                    </select>
                  </FormCard>
                </>
                );
              })()}

              {/* PROJECTS */}
              {activeSection === 'projects' && (
                <>
                  <FormCard>
                    <div className="flex items-center justify-between">
                      <div className="flex-1">
                        <label className="text-[13px] block mb-1" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>
                          Enabled
                        </label>
                        <p className="text-[12px]" style={{ color: '#666' }}>
                          Enable project management features
                        </p>
                      </div>
                      {/* TODO: Wire toggle to PATCH /api/settings/projects */}
                      <Toggle checked={bool(projects, 'enabled', false)} onChange={() => {}} />
                    </div>
                  </FormCard>
                </>
              )}

              {/* PACKS & SKILLS */}
              {activeSection === 'packs-skills' && (
                <>
                  <div className="mb-4">
                    <h3 className="text-[14px] mb-3" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Feature Packs</h3>
                    <p className="text-[12px] mb-4" style={{ color: '#666' }}>Select feature packs to enable</p>
                    <div className="flex flex-wrap gap-2">
                      {(() => {
                        const enabledPacks = arr(packs, 'enabled').map(String);
                        // Show enabled packs from config; if empty, show a placeholder
                        if (enabledPacks.length === 0) {
                          return (
                            <span className="text-[12px]" style={{ color: '#666' }}>No packs enabled</span>
                          );
                        }
                        return enabledPacks.map((pack) => (
                          <button
                            key={pack}
                            className="px-3 py-1.5 rounded text-[12px] border transition-all"
                            style={{
                              backgroundColor: 'var(--codex-accent-dim)',
                              borderColor: 'var(--codex-accent)',
                              color: 'var(--codex-accent)'
                            }}
                            // TODO: Wire onClick to toggle pack via PATCH /api/settings/packs
                          >
                            {pack}
                          </button>
                        ));
                      })()}
                    </div>
                  </div>

                  <FormCard title="Enabled Skills" description="Comma-separated list of enabled skills">
                    <TextInput value={arr(packs, 'enabledSkills').map(String).join(', ')} />
                  </FormCard>

                  <div className="mb-4 mt-8">
                    <h3 className="text-[14px] mb-4" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Plugins</h3>
                  </div>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enabled</label>
                      {/* TODO: Wire toggle to PATCH /api/settings/plugins */}
                      <Toggle checked={bool(plugins, 'enabled', false)} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Registry URL">
                    <TextInput value={str(plugins, 'registryUrl', str(plugins, 'registry', ''))} />
                  </FormCard>
                  <FormCard title="Sandbox Memory (MB)">
                    <TextInput value={str(plugins, 'sandboxMemoryMb', str(plugins, 'sandboxMemory', ''))} />
                  </FormCard>
                  <FormCard>
                    <div className="flex items-center justify-between">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Allow Network by Default</label>
                      {/* TODO: Wire toggle to PATCH /api/settings/plugins */}
                      <Toggle checked={bool(plugins, 'allowNetworkByDefault', bool(plugins, 'allowNetwork', false))} onChange={() => {}} />
                    </div>
                  </FormCard>
                </>
              )}

              {/* PLUGINS */}
              {activeSection === 'plugins' && (
                <>
                  <FormCard>
                    <div className="flex items-center justify-between mb-3">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Enabled</label>
                      {/* TODO: Wire toggle to PATCH /api/settings/plugins */}
                      <Toggle checked={bool(plugins, 'enabled', false)} onChange={() => {}} />
                    </div>
                  </FormCard>
                  <FormCard title="Registry URL">
                    <TextInput value={str(plugins, 'registryUrl', str(plugins, 'registry', ''))} />
                  </FormCard>
                  <FormCard title="Sandbox Memory (MB)">
                    <TextInput value={str(plugins, 'sandboxMemoryMb', str(plugins, 'sandboxMemory', ''))} />
                  </FormCard>
                  <FormCard>
                    <div className="flex items-center justify-between">
                      <label className="text-[13px]" style={{ color: 'var(--codex-fg)', fontWeight: 500 }}>Allow Network by Default</label>
                      {/* TODO: Wire toggle to PATCH /api/settings/plugins */}
                      <Toggle checked={bool(plugins, 'allowNetworkByDefault', bool(plugins, 'allowNetwork', false))} onChange={() => {}} />
                    </div>
                  </FormCard>
                </>
              )}

              {/* Bottom Actions */}
              <div className="flex items-center justify-between pt-6 border-t mt-8" style={{ borderColor: 'var(--codex-border)' }}>
                <button
                  className="text-[13px] transition-colors"
                  style={{ color: 'var(--codex-fg-subtle)' }}
                  onMouseEnter={(e) => e.currentTarget.style.color = 'var(--codex-fg-muted)'}
                  onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
                  // TODO: Wire onClick to confirm + PATCH defaults for the active section
                >
                  Reset to Defaults
                </button>
                <button
                  className="px-6 py-2 rounded-lg text-[14px] transition-colors"
                  style={{
                    backgroundColor: 'var(--codex-accent)',
                    color: 'white'
                  }}
                  onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent-hover)'}
                  onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'var(--codex-accent)'}
                  // TODO: Wire onClick to collect all changed fields and PATCH /api/settings/:section
                >
                  Save Changes
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Right Sidebar */}
      <aside className="w-[260px] border-l overflow-y-auto" style={{
        backgroundColor: 'var(--codex-bg-secondary)',
        borderColor: 'var(--codex-border-subtle)'
      }}>
        {/* Session Info */}
        <div className="border-b" style={{ borderColor: 'var(--codex-border-subtle)' }}>
          <button
            onClick={() => setSessionOpen(!sessionOpen)}
            className="w-full px-4 py-3 flex items-center justify-between transition-colors"
            style={{
              backgroundColor: 'transparent',
              color: 'var(--codex-fg-subtle)'
            }}
            onMouseEnter={(e) => e.currentTarget.style.color = 'var(--codex-fg-muted)'}
            onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
          >
            <span className="text-[10px] uppercase tracking-wider" style={{ fontWeight: 500 }}>
              Session Info
            </span>
            {sessionOpen ?
              <ChevronDown className="w-3.5 h-3.5" strokeWidth={1.5} /> :
              <ChevronRight className="w-3.5 h-3.5" strokeWidth={1.5} />
            }
          </button>
          {sessionOpen && (
            <div className="px-4 pb-4 space-y-3 text-[13px]">
              <div className="flex justify-between items-center">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Session ID</span>
                <span style={{
                  color: 'var(--codex-fg)',
                  fontFamily: 'var(--font-mono)',
                  fontSize: '12px'
                }}>
                  #a8f32e
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Duration</span>
                <span style={{ color: 'var(--codex-fg)' }}>1h 24m</span>
              </div>
              <div className="flex justify-between items-center">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Messages</span>
                <span style={{ color: 'var(--codex-fg)' }}>47</span>
              </div>
            </div>
          )}
        </div>

        {/* Token Usage */}
        <div className="border-b" style={{ borderColor: 'var(--codex-border-subtle)' }}>
          <button
            onClick={() => setTokenOpen(!tokenOpen)}
            className="w-full px-4 py-3 flex items-center justify-between transition-colors"
            style={{
              backgroundColor: 'transparent',
              color: 'var(--codex-fg-subtle)'
            }}
            onMouseEnter={(e) => e.currentTarget.style.color = 'var(--codex-fg-muted)'}
            onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
          >
            <span className="text-[10px] uppercase tracking-wider" style={{ fontWeight: 500 }}>
              Token Usage
            </span>
            {tokenOpen ?
              <ChevronDown className="w-3.5 h-3.5" strokeWidth={1.5} /> :
              <ChevronRight className="w-3.5 h-3.5" strokeWidth={1.5} />
            }
          </button>
          {tokenOpen && (
            <div className="px-4 pb-4 space-y-3">
              <div className="space-y-2">
                <div className="flex justify-between text-[13px] items-center">
                  <span style={{ color: 'var(--codex-fg)' }}>12.4K tokens</span>
                  <span style={{ color: 'var(--codex-fg-subtle)' }}>8%</span>
                </div>
                <div className="h-1 rounded-full overflow-hidden" style={{ backgroundColor: 'var(--codex-bg)' }}>
                  <div className="h-full rounded-full transition-all" style={{
                    width: '8%',
                    backgroundColor: 'var(--codex-accent)'
                  }} />
                </div>
              </div>
              <div className="text-[13px] flex justify-between items-center pt-1">
                <span style={{ color: 'var(--codex-fg-subtle)' }}>Cost</span>
                <span style={{
                  color: 'var(--codex-accent)',
                  fontFamily: 'var(--font-mono)'
                }}>
                  $0.05
                </span>
              </div>
            </div>
          )}
        </div>

        {/* Config Status */}
        <div>
          <button
            onClick={() => setConfigOpen(!configOpen)}
            className="w-full px-4 py-3 flex items-center justify-between transition-colors"
            style={{
              backgroundColor: 'transparent',
              color: 'var(--codex-fg-subtle)'
            }}
            onMouseEnter={(e) => e.currentTarget.style.color = 'var(--codex-fg-muted)'}
            onMouseLeave={(e) => e.currentTarget.style.color = 'var(--codex-fg-subtle)'}
          >
            <span className="text-[10px] uppercase tracking-wider" style={{ fontWeight: 500 }}>
              Config Status
            </span>
            {configOpen ?
              <ChevronDown className="w-3.5 h-3.5" strokeWidth={1.5} /> :
              <ChevronRight className="w-3.5 h-3.5" strokeWidth={1.5} />
            }
          </button>
          {configOpen && (
            <div className="px-4 pb-4 space-y-3 text-[13px]">
              <div>
                <div className="text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>
                  Config file
                </div>
                <div style={{
                  color: 'var(--codex-fg-muted)',
                  fontFamily: 'var(--font-mono)',
                  fontSize: '11px'
                }}>
                  {str(config ?? {}, 'dataDir', '~/.klyntbot')}/config.json
                </div>
              </div>
              <div>
                <div className="text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>
                  Last modified
                </div>
                <div style={{ color: 'var(--codex-fg)' }}>
                  {loading ? '...' : (error ? 'Unknown' : 'Loaded')}
                </div>
              </div>
              <div>
                <div className="text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>
                  Data directory
                </div>
                <div style={{
                  color: 'var(--codex-fg-muted)',
                  fontFamily: 'var(--font-mono)',
                  fontSize: '11px'
                }}>
                  {str(config ?? {}, 'dataDir', '~/.klyntbot')}
                </div>
              </div>
              <div>
                <div className="text-[11px] mb-1" style={{ color: 'var(--codex-fg-subtle)' }}>
                  Sections loaded
                </div>
                <div style={{ color: 'var(--codex-fg)' }}>
                  {config ? Object.keys(config).length : 0}
                </div>
              </div>
            </div>
          )}
        </div>
      </aside>
    </>
  );
}
