import { FieldLayout } from "./FieldLayout";

type Option = { value: string; label: string };

type Props = {
  label: string;
  description?: string;
  value: string;
  options: Option[];
  onChange: (v: string) => void;
};

export function SelectField({ label, description, value, options, onChange }: Props) {
  return (
    <FieldLayout label={label} description={description}>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="mt-1 rounded-md border border-[var(--border-subtle)] bg-[var(--surface-control)] px-3 py-2 text-[var(--fs-sm)] text-[var(--text-strong)] focus:outline-none focus:ring-2 focus:ring-[var(--border-accent)]"
      >
        {options.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>
    </FieldLayout>
  );
}
