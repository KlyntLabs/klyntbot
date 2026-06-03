import type { ReactNode } from "react";
import { cn } from "@/utils/cn";

type MainTopbarProps = {
  leftNode: ReactNode;
  actionsNode?: ReactNode;
  className?: string;
};

export function MainTopbar({ leftNode, actionsNode, className }: MainTopbarProps) {
  return (
    <div
      className={cn(
        "flex items-center justify-between gap-3",
        "h-[var(--main-topbar-height,44px)] min-h-[var(--main-topbar-height,44px)]",
        "px-[var(--topbar-compact-padding,16px)]",
        "border-b border-border-subtle",
        "bg-surface-topbar",
        className,
      )}
      data-tauri-drag-region
    >
      <div className="flex items-center gap-2 min-w-0">{leftNode}</div>
      <div className="flex items-center gap-1 shrink-0">{actionsNode ?? null}</div>
    </div>
  );
}
