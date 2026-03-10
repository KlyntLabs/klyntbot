import { cn } from "@shared/lib/cn";
import type { ReactNode } from "react";

export interface FormFieldProps {
  label?: string;
  description?: string;
  error?: string;
  required?: boolean;
  className?: string;
  children: ReactNode;
}

export function FormField({
  label,
  description,
  error,
  required,
  className,
  children,
}: FormFieldProps) {
  return (
    <div className={cn("space-y-1.5", className)}>
      {label && (
        <label className="text-xs font-medium text-secondary">
          {label}
          {required && (
            <span className="text-destructive ml-0.5">*</span>
          )}
        </label>
      )}
      {description && (
        <p className="text-xs text-muted">{description}</p>
      )}
      {children}
      {error && (
        <p className="text-xs text-destructive">{error}</p>
      )}
    </div>
  );
}
