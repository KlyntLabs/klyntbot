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
        className="w-7 h-7 rounded-lg glass-button flex items-center justify-center text-muted-foreground hover:text-foreground"
      >
        <ChevronLeft className="w-4 h-4" strokeWidth={1.5} />
      </button>
      <button
        type="button"
        onClick={onNext}
        aria-label="Next period"
        className="w-7 h-7 rounded-lg glass-button flex items-center justify-center text-muted-foreground hover:text-foreground"
      >
        <ChevronRight className="w-4 h-4" strokeWidth={1.5} />
      </button>
      <span className="text-[13px] font-medium text-foreground ml-1">{label}</span>
    </div>
  );
}
