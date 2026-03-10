import { cn } from "@shared/lib/cn";
import { type ReactNode, useState } from "react";

export interface TooltipProps {
  content: ReactNode;
  side?: "top" | "bottom" | "left" | "right";
  children: ReactNode;
  className?: string;
}

export function Tooltip({ content, side = "top", children, className }: TooltipProps) {
  const [isVisible, setIsVisible] = useState(false);

  const sideClasses = {
    top: "bottom-full mb-2 left-1/2 -translate-x-1/2",
    bottom: "top-full mt-2 left-1/2 -translate-x-1/2",
    left: "right-full mr-2 top-1/2 -translate-y-1/2",
    right: "left-full ml-2 top-1/2 -translate-y-1/2",
  };

  return (
    <div
      className="relative inline-block"
      onMouseEnter={() => setIsVisible(true)}
      onMouseLeave={() => setIsVisible(false)}
    >
      {children}
      {isVisible && (
        <div
          className={cn(
            "absolute whitespace-nowrap px-2 py-1 text-xs rounded-md",
            "bg-surface-highest text-secondary border border-border",
            "pointer-events-none animate-in fade-in-50 duration-100",
            "z-50 shadow-lg",
            sideClasses[side],
            className,
          )}
        >
          {content}
        </div>
      )}
    </div>
  );
}
