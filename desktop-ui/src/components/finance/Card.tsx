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
  return (
    <div className={cn("bg-surface-low rounded-xl", className)} onClick={onClick}>
      {children}
    </div>
  );
}

export function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <p className="text-[10px] text-dim font-light uppercase tracking-wider mb-2">{children}</p>
  );
}
