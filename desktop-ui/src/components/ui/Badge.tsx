import { cn } from "../../lib/utils";

type BadgeVariant = "priority" | "status" | "area" | "tag";

const colorMaps: Record<string, Record<string, string>> = {
  priority: {
    P1: "bg-red-500/10 text-red-400/80 border-red-500/15",
    P2: "bg-orange-500/10 text-orange-400/80 border-orange-500/15",
    P3: "bg-yellow-500/10 text-yellow-400/80 border-yellow-500/15",
    P4: "bg-blue-500/10 text-blue-400/80 border-blue-500/15",
  },
  status: {
    todo: "bg-info/10 text-info border-info/15",
    doing: "bg-brand/10 text-brand border-brand/15",
    done: "bg-success/10 text-success border-success/15",
  },
  area: {
    work: "bg-info/10 text-info border-info/15",
    personal: "bg-purple/10 text-purple border-purple/15",
  },
};

// Display labels for values that need formatting (e.g. "todo" -> "To Do")
const labelMaps: Record<string, Record<string, string>> = {
  status: {
    todo: "To Do",
    doing: "In Progress",
    done: "Done",
  },
};

const defaultColor = "bg-white/[0.04] text-muted border-white/[0.06]";

interface BadgeProps {
  variant: BadgeVariant;
  value: string;
  color?: string;
  className?: string;
}

export function Badge({ variant, value, color, className }: BadgeProps) {
  const key = value.toLowerCase();

  // If a dynamic color is provided, use inline styles
  if (color) {
    return (
      <span
        className={cn("px-2 py-0.5 text-[11px] font-light rounded-full border", className)}
        style={{
          backgroundColor: `${color}1a`,
          color: color,
          borderColor: `${color}26`,
        }}
      >
        {value}
      </span>
    );
  }

  // Original behavior for non-dynamic badges
  const map = colorMaps[variant];
  const colorClass = map?.[value] ?? map?.[key] ?? defaultColor;
  const label = labelMaps[variant]?.[key] ?? value;

  return (
    <span
      className={cn(
        "px-2 py-0.5 text-[11px] font-light rounded-full border",
        colorClass,
        className,
      )}
    >
      {label}
    </span>
  );
}
