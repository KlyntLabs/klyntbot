import type { ReactNode } from "react";

type Props = {
  label: string;
  description?: string;
  labelExtra?: ReactNode;
  children: ReactNode;
};

export function FieldLayout({ label, description, labelExtra, children }: Props) {
  return (
    <label className="flex flex-col gap-1 py-2">
      <span className="flex items-center justify-between">
        <span className="text-[var(--fs-base)] text-[var(--text-strong)] font-medium">{label}</span>
        {labelExtra}
      </span>
      {description && (
        <span className="text-[var(--fs-xs)] text-[var(--text-subtle)]">{description}</span>
      )}
      {children}
    </label>
  );
}
