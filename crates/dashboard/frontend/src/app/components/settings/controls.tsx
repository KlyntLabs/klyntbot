import { useState } from 'react';
import { Eye, EyeOff, X } from 'lucide-react';

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
  );
}

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
