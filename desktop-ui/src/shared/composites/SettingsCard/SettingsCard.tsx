import type { ReactNode } from "react";

interface SettingsCardProps {
  title: string;
  children: ReactNode;
  className?: string;
}

export function SettingsCard({ title, children, className }: SettingsCardProps) {
  return (
    <div className={`bg-surface-low rounded-lg border border-border p-4 ${className ?? ""}`}>
      <h3 className="text-[13px] font-medium text-secondary mb-3">{title}</h3>
      {children}
    </div>
  );
}
