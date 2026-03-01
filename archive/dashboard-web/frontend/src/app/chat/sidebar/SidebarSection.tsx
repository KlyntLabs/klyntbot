import { ChevronDown, ChevronRight } from 'lucide-react';

export function SidebarSection({
  title,
  open,
  onToggle,
  noBorder,
  children,
}: {
  title: string;
  open: boolean;
  onToggle: () => void;
  noBorder?: boolean;
  children: React.ReactNode;
}) {
  return (
    <div
      className={noBorder ? '' : 'border-b'}
      style={{ borderColor: 'var(--codex-border-subtle)' }}
    >
      <button
        onClick={onToggle}
        className="w-full px-4 py-3 flex items-center justify-between transition-colors"
        style={{
          backgroundColor: 'transparent',
          color: 'var(--codex-fg-subtle)',
        }}
        onMouseEnter={(e) =>
          (e.currentTarget.style.color = 'var(--codex-fg-muted)')
        }
        onMouseLeave={(e) =>
          (e.currentTarget.style.color = 'var(--codex-fg-subtle)')
        }
      >
        <span
          className="text-[10px] uppercase tracking-wider"
          style={{ fontWeight: 500 }}
        >
          {title}
        </span>
        {open ? (
          <ChevronDown className="w-3.5 h-3.5" strokeWidth={1.5} />
        ) : (
          <ChevronRight className="w-3.5 h-3.5" strokeWidth={1.5} />
        )}
      </button>
      {open && children}
    </div>
  );
}

export function SidebarRow({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex justify-between items-center">
      <span style={{ color: 'var(--codex-fg-subtle)' }}>{label}</span>
      <span style={{ color: 'var(--codex-fg)' }}>{children}</span>
    </div>
  );
}
