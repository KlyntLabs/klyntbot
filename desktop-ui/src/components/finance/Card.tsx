import { cn } from "../../lib/utils";

export function Card({
  children,
  className,
  onClick,
}: {
  children: React.ReactNode;
  className?: string;
  onClick?: () => void;
}) {
  if (onClick) {
    return (
      <button
        type="button"
        className={cn("glass-card text-left w-full", className)}
        onClick={onClick}
      >
        {children}
      </button>
    );
  }
  return <div className={cn("glass-card", className)}>{children}</div>;
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
      <h2 className="text-[13px] font-medium text-secondary">{title}</h2>
      <div className="flex items-center gap-2">
        {subtitle && <span className="text-[10px] font-light text-dim">{subtitle}</span>}
        {action}
      </div>
    </div>
  );
}

/** @deprecated Use CardHeader inside a Card instead */
export function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-2">{children}</p>
  );
}
