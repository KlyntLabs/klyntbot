import { cn } from "@shared/lib/utils";
import { ChevronLeft, ChevronRight } from "lucide-react";

export interface DateNavigatorProps {
  label: string;
  onPrev: () => void;
  onNext: () => void;
  className?: string;
}

export function DateNavigator({ label, onPrev, onNext, className }: DateNavigatorProps) {
  return (
    <div className={cn("flex items-center gap-2", className)}>
      <button
        type="button"
        onClick={onPrev}
        aria-label="Previous period"
        className="size-7 rounded-control glass-button flex items-center justify-center text-fg-secondary hover:text-fg"
      >
        <ChevronLeft className="size-4" strokeWidth={1.5} />
      </button>
      <button
        type="button"
        onClick={onNext}
        aria-label="Next period"
        className="size-7 rounded-control glass-button flex items-center justify-center text-fg-secondary hover:text-fg"
      >
        <ChevronRight className="size-4" strokeWidth={1.5} />
      </button>
      <span className="text-ui font-medium text-fg ml-1">{label}</span>
    </div>
  );
}
