import { cn } from "@shared/lib/utils";
import type { ReactNode } from "react";

export interface PageHeaderProps {
  title?: ReactNode;
  nav?: ReactNode;
  actions?: ReactNode;
  className?: string;
}

export function PageHeader({ title, nav, actions, className }: PageHeaderProps) {
  return (
    <header className={cn("flex items-center gap-3 px-5 py-3 border-b border-separator", className)}>
      {title && <div className="text-ui font-semibold text-fg shrink-0">{title}</div>}
      {nav && <nav className="flex items-center gap-1.5">{nav}</nav>}
      <div className="flex-1" />
      {actions && <div className="flex items-center gap-2">{actions}</div>}
    </header>
  );
}
