import { cn } from "@shared/lib/utils";
import type { ReactNode } from "react";

interface SettingsCardProps {
  title: string;
  children: ReactNode;
  className?: string;
}

export function SettingsCard({ title, children, className }: SettingsCardProps) {
  return (
    <div className={cn("bg-card rounded-lg border border-border p-4", className)}>
      <h3 className="text-[13px] font-medium text-muted-foreground mb-3">{title}</h3>
      {children}
    </div>
  );
}
