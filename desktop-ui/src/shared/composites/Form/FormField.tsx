import { cn } from "@shared/lib/utils";
import { type ReactNode, useId } from "react";

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
  const fieldId = useId();

  return (
    <div className={cn("space-y-1.5", className)}>
      {label && (
        <label htmlFor={fieldId} className="text-ui-sm font-medium text-fg-secondary">
          {label}
          {required && <span className="text-status-danger ml-0.5">*</span>}
        </label>
      )}
      {description && <p className="text-ui-sm text-fg-secondary">{description}</p>}
      <div id={fieldId}>{children}</div>
      {error && <p className="text-ui-sm text-status-danger">{error}</p>}
    </div>
  );
}
