import { FieldLayout } from "./FieldLayout";

type Props = {
  label: string;
  description?: string;
  value: string;
  placeholder?: string;
  onChange: (v: string) => void;
  onBlur?: () => void;
};

export function TextField({ label, description, value, placeholder, onChange, onBlur }: Props) {
  return (
    <FieldLayout label={label} description={description}>
      <input
        type="text"
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
        onBlur={onBlur}
        className="mt-1 rounded-md border border-[var(--border-subtle)] bg-[var(--surface-control)] px-3 py-2 text-[var(--fs-sm)] text-[var(--text-strong)] placeholder:text-[var(--text-faint)] focus:outline-none focus:ring-2 focus:ring-[var(--border-accent)]"
      />
    </FieldLayout>
  );
}
