import { useState } from 'react';
import { Outlet, useNavigate, useLocation } from 'react-router';
import {
  MessageSquare,
  CheckSquare,
  FileText,
  Calendar,
  Clock,
  Zap,
  Settings,
  ChevronDown,
  DollarSign,
} from 'lucide-react';

type NavItem = {
  id: string;
  icon: typeof MessageSquare;
  label: string;
  path: string;
};

export function Layout() {
  const navigate = useNavigate();
  const location = useLocation();
  const [modelOpen, setModelOpen] = useState(false);

  const navItems: NavItem[] = [
    { id: 'chat', icon: MessageSquare, label: 'Chat', path: '/' },
    { id: 'tasks', icon: CheckSquare, label: 'Tasks', path: '/tasks' },
    { id: 'plans', icon: FileText, label: 'Plans', path: '/plans' },
    { id: 'calendar', icon: Calendar, label: 'Calendar', path: '/calendar' },
    { id: 'cron', icon: Clock, label: 'Cron', path: '/cron' },
    { id: 'skills', icon: Zap, label: 'Skills', path: '/skills' },
    { id: 'finance', icon: DollarSign, label: 'Finance', path: '/finance' },
  ];

  const isActive = (path: string) => {
    if (path === '/') return location.pathname === '/';
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
        <Outlet />
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
          <button
            className="flex items-center gap-1.5 hover:text-[var(--codex-fg-muted)] transition-colors"
            onClick={() => setModelOpen(!modelOpen)}
          >
            <span>GPT-4</span>
            <ChevronDown className="w-3 h-3" />
          </button>
          <span style={{ color: 'var(--codex-border)' }}>&middot;</span>
          <span>Full permissions</span>
          <span style={{ color: 'var(--codex-border)' }}>&middot;</span>
          <span>Session #a8f32e</span>
        </div>
        <div className="flex items-center gap-3">
          <span style={{ color: 'var(--codex-accent)' }}>$0.05</span>
          <span style={{ color: 'var(--codex-border)' }}>&middot;</span>
          <span>12.4K tokens</span>
        </div>
      </div>
    </div>
  );
}

export default Layout;
