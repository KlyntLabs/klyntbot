import { type ReactNode, useState } from "react";

interface CollapsibleSectionProps {
  title: string;
  icon?: ReactNode;
  count?: number | null;
  defaultOpen?: boolean;
  children: ReactNode;
}

export function CollapsibleSection({
  title,
  icon,
  count,
  defaultOpen = false,
  children,
}: CollapsibleSectionProps) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div>
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="flex items-center gap-2 w-full px-3 py-2 text-sm font-medium text-heading hover:bg-surface-hover rounded-md"
      >
        <span className="text-[10px] text-muted-foreground">{open ? "\u25BE" : "\u25B8"}</span>
        {icon && <span>{icon}</span>}
        <span>{title}</span>
        {count != null && <span className="ml-auto text-xs text-muted-foreground">{count}</span>}
      </button>
      {open && <div className="pl-6 pr-3 pb-2">{children}</div>}
    </div>
  );
}
