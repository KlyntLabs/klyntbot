import { useState, useRef } from 'react';
import { HelpCircle } from 'lucide-react';

interface SettingRowProps {
  label: string;
  description?: string;
  tip?: string;
  children: React.ReactNode;
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
