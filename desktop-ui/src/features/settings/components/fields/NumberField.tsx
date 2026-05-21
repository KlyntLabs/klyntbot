import { FieldLayout } from "./FieldLayout";

type Props = {
  label: string;
  description?: string;
  value: number;
  min?: number;
  max?: number;
  step?: number;
  onChange: (v: number) => void;
  onBlur?: () => void;
};

export function NumberField({
  label,
  description,
  value,
  min,
  max,
  step,
  onChange,
  onBlur,
}: Props) {
  return (
    <FieldLayout label={label} description={description}>
      <input
        type="number"
        value={value}
        min={min}
        max={max}
        step={step}
        onChange={(e) => onChange(Number(e.target.value))}
        onBlur={onBlur}
        className="mt-1 rounded-md border border-[var(--border-subtle)] bg-[var(--surface-control)] px-3 py-2 text-[var(--fs-sm)] text-[var(--text-strong)] placeholder:text-[var(--text-faint)] focus:outline-none focus:ring-2 focus:ring-[var(--border-accent)]"
      />
    </FieldLayout>
  );
}
