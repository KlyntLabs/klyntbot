import { type ReactNode, useCallback, useState } from "react";

interface CollapsibleSectionProps {
  title: string;
  defaultExpanded?: boolean;
  badge?: ReactNode;
  children: ReactNode;
}

export function CollapsibleSection({
  title,
  defaultExpanded = false,
  badge,
  children,
}: CollapsibleSectionProps) {
  const [expanded, setExpanded] = useState(defaultExpanded);

  const toggle = useCallback(() => setExpanded((prev) => !prev), []);

  return (
    <div className="border-b border-border">
      <button
        type="button"
        onClick={toggle}
        aria-expanded={expanded}
        className="flex w-full items-center justify-between px-3 py-2 text-[10px] text-muted-foreground uppercase tracking-wider hover:bg-surface-hover transition-colors"
      >
        <span className="flex items-center gap-2">
          {title}
          {badge}
        </span>
        <span className="text-[10px]">{expanded ? "▾" : "▸"}</span>
      </button>
      {expanded && <div className="px-3 pb-3">{children}</div>}
    </div>
  );
}
