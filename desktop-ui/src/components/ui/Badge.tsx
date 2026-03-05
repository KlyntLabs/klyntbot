import { cn } from "../../lib/utils";

type BadgeVariant = "priority" | "status" | "area" | "tag";

const colorMaps: Record<string, Record<string, string>> = {
  priority: {
    P1: "bg-red-500/10 text-red-400/80",
    P2: "bg-orange-500/10 text-orange-400/80",
    P3: "bg-yellow-500/10 text-yellow-400/80",
    P4: "bg-blue-500/10 text-blue-400/80",
  },
  status: {
    todo: "bg-info/10 text-info",
    doing: "bg-brand/10 text-brand",
    done: "bg-success/10 text-success",
  },
  area: {
    work: "bg-info/10 text-info",
    personal: "bg-purple/10 text-purple",
  },
};

// Display labels for values that need formatting (e.g. "todo" → "To Do")
const labelMaps: Record<string, Record<string, string>> = {
  status: {
    todo: "To Do",
    doing: "In Progress",
    done: "Done",
  },
};

const defaultColor = "bg-surface-base text-muted";

interface BadgeProps {
  variant: BadgeVariant;
  value: string;
  className?: string;
}

export function Badge({ variant, value, className }: BadgeProps) {
  const key = value.toLowerCase();
  const map = colorMaps[variant];
  const colorClass = map?.[value] ?? map?.[key] ?? defaultColor;
  const label = labelMaps[variant]?.[key] ?? value;

  return (
    <span className={cn("px-2 py-0.5 text-[11px] font-light rounded", colorClass, className)}>
      {label}
    </span>
  );
}
