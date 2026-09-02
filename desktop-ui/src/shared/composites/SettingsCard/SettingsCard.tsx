import { cn } from "@klyntbot/design-system";
import type { ReactNode } from "react";

interface SettingsCardProps {
  title: string;
  children: ReactNode;
  className?: string;
}

export function SettingsCard({ title, children, className }: SettingsCardProps) {
  return (
    <div className={cn("island rounded-panel p-4", className)}>
      <h3 className="text-ui font-medium text-fg-secondary mb-3">{title}</h3>
      {children}
    </div>
  );
}
