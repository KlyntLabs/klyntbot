import type { ReactNode } from "react";

interface Props {
  variant?: "brand" | "muted" | "destructive";
  className?: string;
  children: ReactNode;
}

export function Badge({ variant = "muted", className, children }: Props) {
  return (
    <span className={`tray-badge variant-${variant}${className ? ` ${className}` : ""}`}>
      {children}
    </span>
  );
}
