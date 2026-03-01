import { useState } from 'react';
import { ChevronDown, ChevronRight } from 'lucide-react';

interface SettingSectionProps {
  title: string;
  description?: string;
  defaultOpen?: boolean;
  children: React.ReactNode;
}

export function SettingSection({ title, description, defaultOpen = true, children }: SettingSectionProps) {
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
