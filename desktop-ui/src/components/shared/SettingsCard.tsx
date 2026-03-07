import type { ReactNode } from "react";

interface SettingsCardProps {
  title: string;
  children: ReactNode;
  className?: string;
}

export function SettingsCard({ title, children, className }: SettingsCardProps) {
  return (
    <div className={`bg-white/[0.04] rounded-lg border border-white/[0.08] p-4 ${className ?? ""}`}>
      <h3 className="text-[13px] font-medium text-secondary mb-3">{title}</h3>
      {children}
    </div>
  );
}
