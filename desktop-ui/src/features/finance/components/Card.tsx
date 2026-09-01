import { cn } from "@shared/lib/utils";

export function Card({
  children,
  className,
  compact,
  onClick,
}: {
  children: React.ReactNode;
  className?: string;
  /** Use tighter radius for sidebar / small cards */
  compact?: boolean;
  onClick?: () => void;
}) {
  const base = compact ? "glass-card-sm" : "glass-card";
  if (onClick) {
    return (
      <button type="button" className={cn(base, "text-left w-full", className)} onClick={onClick}>
        {children}
      </button>
    );
  }
  return <div className={cn(base, className)}>{children}</div>;
}

export function CardHeader({
  title,
  subtitle,
  action,
}: {
  title: string;
  subtitle?: string;
  action?: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between mb-3">
      <h2 className="text-[13px] font-medium text-muted-foreground">{title}</h2>
      <div className="flex items-center gap-2">
        {subtitle && <span className="text-2xs font-light text-dim">{subtitle}</span>}
        {action}
      </div>
    </div>
  );
}

/** @deprecated Use CardHeader inside a Card instead */
export function SectionLabel({ children }: { children: React.ReactNode }) {
  return <p className="text-2xs text-dim font-light uppercase tracking-wider mb-2">{children}</p>;
}
