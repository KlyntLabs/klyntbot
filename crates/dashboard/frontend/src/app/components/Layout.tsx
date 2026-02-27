import { useState, useRef, useEffect, useCallback } from 'react';
import { Outlet, useNavigate, useLocation } from 'react-router';
import {
  MessageSquare,
  History,
  CheckSquare,
  FolderKanban,
  FileText,
  Calendar,
  Clock,
  Zap,
  Settings,
  ChevronDown,
  DollarSign,
  Check,
} from 'lucide-react';
import { useApi } from '../../lib/hooks/useApi';
import { apiFetch } from '../../lib/api';
import type { AgentStatus } from '../../lib/types';
import { PROVIDER_MODELS, displayName } from './settings/model-data';

type NavItem = {
  id: string;
  icon: typeof MessageSquare;
  label: string;
  path: string;
};

const PERMISSION_LEVELS = [
  { value: 'full', label: 'Full permissions', desc: 'All tools allowed' },
  { value: 'admin', label: 'Admin', desc: 'All tools, all channels' },
  { value: 'elevated', label: 'Elevated', desc: 'Most tools allowed' },
  { value: 'standard', label: 'Standard', desc: 'Safe tools only' },
  { value: 'readOnly', label: 'Read only', desc: 'No write operations' },
];

export function Layout() {
  const navigate = useNavigate();
  const location = useLocation();
  const [providerOpen, setProviderOpen] = useState(false);
  const [modelOpen, setModelOpen] = useState(false);
  const [permOpen, setPermOpen] = useState(false);
  const { data: status, refetch: refetchStatus } = useApi<AgentStatus>('/api/status');

  const providerRef = useRef<HTMLDivElement>(null);
  const modelRef = useRef<HTMLDivElement>(null);
  const permRef = useRef<HTMLDivElement>(null);

  // Close dropdowns on outside click
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (providerRef.current && !providerRef.current.contains(e.target as Node)) setProviderOpen(false);
      if (modelRef.current && !modelRef.current.contains(e.target as Node)) setModelOpen(false);
      if (permRef.current && !permRef.current.contains(e.target as Node)) setPermOpen(false);
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, []);

  const activeProvider = status?.provider ?? null;
  const activeModel = status?.model ?? '';
  const configuredProviders = status?.configuredProviders ?? [];
  const activeModels = PROVIDER_MODELS[activeProvider ?? ''] ?? (activeModel ? [activeModel] : []);

  const switchProvider = useCallback(async (provider: string) => {
    // Pick first model of the new provider as default
    const models = PROVIDER_MODELS[provider] ?? [`${provider}/default`];
    try {
      await apiFetch('/api/settings/agents', { method: 'PATCH', body: { defaults: { provider, model: models[0] } } });
      refetchStatus();
      setProviderOpen(false);
    } catch { /* ignore */ }
  }, [refetchStatus]);

  const switchModel = useCallback(async (model: string) => {
    try {
      await apiFetch('/api/settings/agents', { method: 'PATCH', body: { defaults: { model } } });
      refetchStatus();
      setModelOpen(false);
    } catch { /* ignore */ }
  }, [refetchStatus]);

  const switchPermission = useCallback(async (level: string) => {
    try {
      if (level === 'full') {
        await apiFetch('/api/settings/tools', { method: 'PATCH', body: { permissions: null } });
      } else {
        await apiFetch('/api/settings/tools', { method: 'PATCH', body: { permissions: { defaultLevel: level } } });
      }
      refetchStatus();
      setPermOpen(false);
    } catch { /* ignore */ }
  }, [refetchStatus]);

  const shortModel = activeModel.includes('/') ? activeModel.split('/').pop()! : activeModel;
  const displayPerm = status?.permissionLevel
    ? `${status.permissionLevel[0].toUpperCase()}${status.permissionLevel.slice(1)} permissions`
    : '—';

  const navItems: NavItem[] = [
    { id: 'chat', icon: MessageSquare, label: 'Chat', path: '/' },
    { id: 'sessions', icon: History, label: 'Sessions', path: '/sessions' },
    { id: 'tasks', icon: CheckSquare, label: 'Tasks', path: '/tasks' },
    { id: 'projects', icon: FolderKanban, label: 'Projects', path: '/projects' },
    { id: 'plans', icon: FileText, label: 'Plans', path: '/plans' },
    { id: 'calendar', icon: Calendar, label: 'Calendar', path: '/calendar' },
    { id: 'cron', icon: Clock, label: 'Cron', path: '/cron' },
    { id: 'skills', icon: Zap, label: 'Skills', path: '/skills' },
    { id: 'finance', icon: DollarSign, label: 'Finance', path: '/finance' },
  ];

  const isActive = (path: string) => {
    if (path === '/') return location.pathname === '/' || location.pathname.startsWith('/chat/');
    return location.pathname.startsWith(path);
  };

  return (
    <div
      className="h-screen flex flex-col overflow-hidden"
      style={{
        backgroundColor: 'var(--codex-bg)',
        color: 'var(--codex-fg)',
        fontFamily: 'var(--font-ui)',
      }}
    >
      {/* macOS Title Bar */}
      <div
        className="h-[44px] flex items-center px-4 border-b"
        style={{
          backgroundColor: 'var(--codex-bg)',
          borderColor: 'var(--codex-border-subtle)',
        }}
      >
        <div className="flex gap-2">
          <div
            className="w-3 h-3 rounded-full"
            style={{ backgroundColor: '#ec695e' }}
          />
          <div
            className="w-3 h-3 rounded-full"
            style={{ backgroundColor: '#f4bf4f' }}
          />
          <div
            className="w-3 h-3 rounded-full"
            style={{ backgroundColor: '#61c554' }}
          />
        </div>
        <div
          className="flex-1 text-center text-[13px]"
          style={{
            color: 'var(--codex-fg-muted)',
            fontWeight: 400,
          }}
        >
          klyntbot
        </div>
      </div>

      {/* Main Content */}
      <div className="flex-1 flex overflow-hidden">
        {/* Left Navigation Rail */}
        <nav
          aria-label="Main navigation"
          className="w-[48px] flex flex-col items-center py-3 border-r"
          style={{
            backgroundColor: 'var(--codex-bg-nav)',
            borderColor: 'var(--codex-border-subtle)',
          }}
        >
          <div className="flex-1 flex flex-col gap-0.5">
            {navItems.map((item) => {
              const active = isActive(item.path);
              return (
                <button
                  key={item.id}
                  onClick={() => navigate(item.path)}
                  aria-label={item.label}
                  aria-current={active ? 'page' : undefined}
                  className="w-full h-9 flex items-center justify-center relative group"
                  style={{
                    color: active
                      ? 'var(--codex-fg)'
                      : 'var(--codex-fg-subtle)',
                  }}
                  onMouseEnter={(e) => {
                    if (!active) {
                      e.currentTarget.style.color = 'var(--codex-fg-muted)';
                    }
                  }}
                  onMouseLeave={(e) => {
                    if (!active) {
                      e.currentTarget.style.color = 'var(--codex-fg-subtle)';
                    }
                  }}
                >
                  {/* Active indicator - subtle left border */}
                  {active && (
                    <div
                      className="absolute left-0 top-1/2 -translate-y-1/2 w-[2px] h-4 rounded-r"
                      style={{
                        backgroundColor: 'var(--codex-accent)',
                      }}
                    />
                  )}
                  <item.icon className="w-[18px] h-[18px]" strokeWidth={1.5} />

                  {/* Tooltip */}
                  <div
                    className="absolute left-full ml-2 px-2 py-1 rounded opacity-0 group-hover:opacity-100 pointer-events-none transition-opacity whitespace-nowrap text-xs"
                    style={{
                      backgroundColor: 'var(--codex-bg-tertiary)',
                      color: 'var(--codex-fg)',
                      border: '1px solid var(--codex-border)',
                    }}
                  >
                    {item.label}
                  </div>
                </button>
              );
            })}
          </div>

          {/* Settings at bottom */}
          <button
            onClick={() => navigate('/settings')}
            aria-label="Settings"
            aria-current={location.pathname === '/settings' ? 'page' : undefined}
            className="w-full h-9 flex items-center justify-center relative group mt-2 border-t pt-2"
            style={{
              color:
                location.pathname === '/settings'
                  ? 'var(--codex-fg)'
                  : 'var(--codex-fg-subtle)',
              borderColor: 'var(--codex-border-subtle)',
            }}
            onMouseEnter={(e) => {
              if (location.pathname !== '/settings') {
                e.currentTarget.style.color = 'var(--codex-fg-muted)';
              }
            }}
            onMouseLeave={(e) => {
              if (location.pathname !== '/settings') {
                e.currentTarget.style.color = 'var(--codex-fg-subtle)';
              }
            }}
          >
            {/* Active indicator */}
            {location.pathname === '/settings' && (
              <div
                className="absolute left-0 top-1/2 -translate-y-1/2 w-[2px] h-4 rounded-r"
                style={{
                  backgroundColor: 'var(--codex-accent)',
                }}
              />
            )}
            <Settings className="w-[18px] h-[18px]" strokeWidth={1.5} />

            {/* Tooltip */}
            <div
              className="absolute left-full ml-2 px-2 py-1 rounded opacity-0 group-hover:opacity-100 pointer-events-none transition-opacity whitespace-nowrap text-xs"
              style={{
                backgroundColor: 'var(--codex-bg-tertiary)',
                color: 'var(--codex-fg)',
                border: '1px solid var(--codex-border)',
              }}
            >
              Settings
            </div>
          </button>
        </nav>

        {/* Page Content */}
        <main className="flex-1 flex min-w-0 overflow-hidden">
          <Outlet />
        </main>
      </div>

      {/* Bottom Status Bar */}
      <div
        className="h-[28px] px-4 flex items-center justify-between border-t text-[11px]"
        style={{
          backgroundColor: 'var(--codex-bg)',
          borderColor: 'var(--codex-border-subtle)',
          color: 'var(--codex-fg-subtle)',
          fontFamily: 'var(--font-mono)',
        }}
      >
        <div className="flex items-center gap-3">
          {/* Provider dropdown */}
          <div ref={providerRef}>
            <button
              className="flex items-center gap-1.5 hover:text-[var(--codex-fg-muted)] transition-colors"
              onClick={() => { setProviderOpen(!providerOpen); setModelOpen(false); setPermOpen(false); }}
            >
              <span>{activeProvider ? displayName(activeProvider) : '—'}</span>
              <ChevronDown className="w-3 h-3" style={{ transform: providerOpen ? 'rotate(180deg)' : undefined, transition: 'transform 0.15s' }} />
            </button>
          </div>
          {providerOpen && providerRef.current && (() => {
            const r = providerRef.current!.getBoundingClientRect();
            return (
              <div
                className="fixed min-w-[180px] rounded-lg border py-1 shadow-xl"
                style={{ left: r.left, bottom: window.innerHeight - r.top + 4, backgroundColor: 'var(--codex-bg-tertiary)', borderColor: 'var(--codex-border)', zIndex: 9999 }}
              >
                {configuredProviders.length === 0 ? (
                  <div className="px-3 py-2 text-[11px]" style={{ color: 'var(--codex-fg-subtle)' }}>
                    No providers configured.<br />Add API keys in Settings.
                  </div>
                ) : configuredProviders.map(prov => {
                  const isActive = activeProvider === prov;
                  return (
                    <button
                      key={prov}
                      className="w-full px-3 py-1.5 flex items-center gap-2 text-left text-[11px] transition-colors"
                      style={{ color: isActive ? 'var(--codex-accent)' : 'var(--codex-fg-muted)' }}
                      onMouseEnter={e => { e.currentTarget.style.backgroundColor = 'var(--codex-bg-secondary)'; }}
                      onMouseLeave={e => { e.currentTarget.style.backgroundColor = 'transparent'; }}
                      onClick={() => switchProvider(prov)}
                    >
                      <Check className="w-3 h-3 flex-shrink-0" style={{ opacity: isActive ? 1 : 0 }} strokeWidth={2} />
                      <span>{displayName(prov)}</span>
                    </button>
                  );
                })}
              </div>
            );
          })()}

          <span style={{ color: 'var(--codex-border)' }}>/</span>

          {/* Model dropdown */}
          <div ref={modelRef}>
            <button
              className="flex items-center gap-1.5 hover:text-[var(--codex-fg-muted)] transition-colors"
              onClick={() => { setModelOpen(!modelOpen); setProviderOpen(false); setPermOpen(false); }}
            >
              <span>{shortModel || '—'}</span>
              <ChevronDown className="w-3 h-3" style={{ transform: modelOpen ? 'rotate(180deg)' : undefined, transition: 'transform 0.15s' }} />
            </button>
          </div>
          {modelOpen && modelRef.current && (() => {
            const r = modelRef.current!.getBoundingClientRect();
            return (
              <div
                className="fixed min-w-[220px] rounded-lg border py-1 shadow-xl"
                style={{ left: r.left, bottom: window.innerHeight - r.top + 4, backgroundColor: 'var(--codex-bg-tertiary)', borderColor: 'var(--codex-border)', zIndex: 9999 }}
              >
                {activeModels.map(m => {
                  const isActive = activeModel === m;
                  const label = m.includes('/') ? m.split('/').pop()! : m;
                  return (
                    <button
                      key={m}
                      className="w-full px-3 py-1.5 flex items-center gap-2 text-left text-[11px] transition-colors"
                      style={{ color: isActive ? 'var(--codex-accent)' : 'var(--codex-fg-muted)' }}
                      onMouseEnter={e => { e.currentTarget.style.backgroundColor = 'var(--codex-bg-secondary)'; }}
                      onMouseLeave={e => { e.currentTarget.style.backgroundColor = 'transparent'; }}
                      onClick={() => switchModel(m)}
                    >
                      <Check className="w-3 h-3 flex-shrink-0" style={{ opacity: isActive ? 1 : 0 }} strokeWidth={2} />
                      <span style={{ fontFamily: 'var(--font-mono)' }}>{label}</span>
                    </button>
                  );
                })}
              </div>
            );
          })()}

          <span style={{ color: 'var(--codex-border)' }}>&middot;</span>

          {/* Permission level dropdown */}
          <div ref={permRef}>
            <button
              className="flex items-center gap-1.5 hover:text-[var(--codex-fg-muted)] transition-colors"
              onClick={() => { setPermOpen(!permOpen); setProviderOpen(false); setModelOpen(false); }}
            >
              <span>{displayPerm}</span>
              <ChevronDown className="w-3 h-3" style={{ transform: permOpen ? 'rotate(180deg)' : undefined, transition: 'transform 0.15s' }} />
            </button>
          </div>
          {permOpen && permRef.current && (() => {
            const r = permRef.current!.getBoundingClientRect();
            return (
              <div
                className="fixed min-w-[220px] rounded-lg border py-1 shadow-xl"
                style={{ left: r.left, bottom: window.innerHeight - r.top + 4, backgroundColor: 'var(--codex-bg-tertiary)', borderColor: 'var(--codex-border)', zIndex: 9999 }}
              >
                {PERMISSION_LEVELS.map(({ value, label, desc }) => {
                  const isActive = status?.permissionLevel === value;
                  return (
                    <button
                      key={value}
                      className="w-full px-3 py-1.5 flex items-center gap-2 text-left text-[11px] transition-colors"
                      style={{ color: isActive ? 'var(--codex-accent)' : 'var(--codex-fg-muted)' }}
                      onMouseEnter={e => { e.currentTarget.style.backgroundColor = 'var(--codex-bg-secondary)'; }}
                      onMouseLeave={e => { e.currentTarget.style.backgroundColor = 'transparent'; }}
                      onClick={() => switchPermission(value)}
                    >
                      <Check className="w-3 h-3 flex-shrink-0" style={{ opacity: isActive ? 1 : 0 }} strokeWidth={2} />
                      <div>
                        <div>{label}</div>
                        <div className="text-[10px]" style={{ color: 'var(--codex-fg-subtle)' }}>{desc}</div>
                      </div>
                    </button>
                  );
                })}
              </div>
            );
          })()}

          <span style={{ color: 'var(--codex-border)' }}>&middot;</span>
          <span>{status?.version ?? '—'}</span>
        </div>
        <div className="flex items-center gap-3">
          <span>{status?.storage.taskCount ?? 0} tasks</span>
          <span style={{ color: 'var(--codex-border)' }}>&middot;</span>
          <span>{status?.storage.sessionCount ?? 0} sessions</span>
        </div>
      </div>
    </div>
  );
}

export default Layout;
