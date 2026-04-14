import { ArrowLeft } from "lucide-react";
import type { ReactNode } from "react";

interface BrainCardProps {
  title: string;
  subtitle: string;
  icon: ReactNode;
  accentClass: string;
  summary: ReactNode;
  detail: ReactNode;
  expanded: boolean;
  onExpand: () => void;
  onCollapse: () => void;
  actions?: ReactNode;
}

export function BrainCard({
  title,
  subtitle,
  icon,
  accentClass,
  summary,
  detail,
  expanded,
  onExpand,
  onCollapse,
  actions,
}: BrainCardProps) {
  if (expanded) {
    return (
      <div className="animate-in fade-in duration-200">
        <div className="flex items-center gap-3 mb-5">
          <button
            type="button"
            onClick={onCollapse}
            className="size-7 rounded-lg bg-surface-low flex items-center justify-center text-muted-foreground hover:text-foreground transition-colors"
          >
            <ArrowLeft className="size-3.5" />
          </button>
          <div className="flex items-center gap-2.5 flex-1 min-w-0">
            <div className={`size-8 rounded-lg flex items-center justify-center ${accentClass}`}>
              {icon}
            </div>
            <div className="min-w-0">
              <h2 className="text-sm font-semibold text-foreground">{title}</h2>
              <p className="text-2xs text-muted-foreground">{subtitle}</p>
            </div>
          </div>
          {actions && <div className="flex items-center gap-2">{actions}</div>}
        </div>
        {detail}
      </div>
    );
  }

  return (
    <button
      type="button"
      onClick={onExpand}
      className="w-full text-left bg-surface-lowest border border-border rounded-xl p-5 hover:border-border-hover transition-colors duration-200 cursor-pointer"
    >
      <div className="flex items-center gap-2.5 mb-3.5">
        <div className={`size-8 rounded-lg flex items-center justify-center ${accentClass}`}>
          {icon}
        </div>
        <div className="min-w-0">
          <h3 className="text-[13px] font-semibold text-foreground">{title}</h3>
          <p className="text-2xs text-muted-foreground">{subtitle}</p>
        </div>
      </div>
      {summary}
    </button>
  );
}
