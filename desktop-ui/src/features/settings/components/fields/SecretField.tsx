import { FieldLayout } from "./FieldLayout";

type Props = {
  label: string;
  description?: string;
  configured: boolean;
  onChange: (v: string) => void;
  onBlur?: () => void;
};

export function SecretField({ label, description, configured, onChange, onBlur }: Props) {
  return (
    <FieldLayout
      label={label}
      description={description}
      labelExtra={
        configured ? (
          <span className="rounded-full bg-[var(--surface-card-muted)] px-2 py-0.5 text-[var(--fs-2xs)] text-[var(--text-subtle)] font-medium">
            Configured
          </span>
        ) : null
      }
    >
      <input
        type="password"
        placeholder={configured ? "••••••••" : "Enter value…"}
        onChange={(e) => onChange(e.target.value)}
        onBlur={onBlur}
        className="mt-1 rounded-md border border-[var(--border-subtle)] bg-[var(--surface-control)] px-3 py-2 text-[var(--fs-sm)] text-[var(--text-strong)] placeholder:text-[var(--text-faint)] focus:outline-none focus:ring-2 focus:ring-[var(--border-accent)]"
      />
    </FieldLayout>
  );
}
