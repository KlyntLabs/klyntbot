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
        className={cn("bg-surface-low rounded-xl text-left w-full", className)}
        onClick={onClick}
      >
        {children}
      </button>
    );
  }
  return <div className={cn("bg-surface-low rounded-xl", className)}>{children}</div>;
}

export function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-2">{children}</p>
  );
}
