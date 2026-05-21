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

export function SliderField({
  label,
  description,
  value,
  min = 0,
  max = 100,
  step = 1,
  onChange,
  onBlur,
}: Props) {
  return (
    <FieldLayout
      label={label}
      description={description}
      labelExtra={
        <span className="text-[var(--fs-sm)] text-[var(--text-subtle)] tabular-nums">{value}</span>
      }
    >
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        onBlur={onBlur}
        className="mt-1 w-full accent-[var(--border-accent)]"
      />
    </FieldLayout>
  );
}
