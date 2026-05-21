type Props = {
  label: string;
  description?: string;
  value: boolean;
  onChange: (v: boolean) => void;
};

export function ToggleField({ label, description, value, onChange }: Props) {
  return (
    <label className="flex items-center justify-between gap-4 py-2">
      <span className="flex flex-col min-w-0">
        <span className="text-[var(--fs-base)] text-[var(--text-strong)] font-medium">{label}</span>
        {description && (
          <span className="text-[var(--fs-xs)] text-[var(--text-subtle)]">{description}</span>
        )}
      </span>
      <button
        type="button"
        role="switch"
        aria-checked={value}
        onClick={() => onChange(!value)}
        className={`relative h-5 w-9 rounded-full transition-colors shrink-0 ${
          value ? "bg-[var(--border-accent)]" : "bg-[var(--surface-control-disabled)]"
        }`}
      >
        <span
          className={`absolute top-0.5 left-0.5 block h-4 w-4 rounded-full bg-[var(--text-strong)] transition-transform ${
            value ? "translate-x-4" : "translate-x-0"
          }`}
        />
      </button>
    </label>
  );
}
