import { cn } from "@shared/lib/cn";
import type { ReactNode } from "react";

export interface FormSectionProps {
  title: string;
  description?: string;
  className?: string;
  children: ReactNode;
}

export function FormSection({ title, description, className, children }: FormSectionProps) {
  return (
    <div className={cn("space-y-4", className)}>
      <div>
        <h3 className="text-sm font-medium text-primary">{title}</h3>
        {description && <p className="text-xs text-muted mt-0.5">{description}</p>}
      </div>
      <div className="space-y-3">{children}</div>
    </div>
  );
}
