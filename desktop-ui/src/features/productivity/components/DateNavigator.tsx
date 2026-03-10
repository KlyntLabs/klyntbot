import { Calendar, ChevronLeft, ChevronRight } from "lucide-react";

interface DateNavigatorProps {
  label: string;
  onPrev: () => void;
  onNext: () => void;
  onToday: () => void;
}

export function DateNavigator({ label, onPrev, onNext, onToday }: DateNavigatorProps) {
  return (
    <div className="flex items-center gap-2">
      <button
        type="button"
        onClick={onPrev}
        aria-label="Previous period"
        className="w-7 h-7 rounded-lg glass-button flex items-center justify-center text-muted hover:text-secondary"
      >
        <ChevronLeft className="w-4 h-4" strokeWidth={1.5} />
      </button>
      <button
        type="button"
        onClick={onToday}
        aria-label="Go to today"
        className="w-7 h-7 rounded-lg glass-button flex items-center justify-center text-muted hover:text-secondary"
      >
        <Calendar className="w-3.5 h-3.5" strokeWidth={1.5} />
      </button>
      <button
        type="button"
        onClick={onNext}
        aria-label="Next period"
        className="w-7 h-7 rounded-lg glass-button flex items-center justify-center text-muted hover:text-secondary"
      >
        <ChevronRight className="w-4 h-4" strokeWidth={1.5} />
      </button>
      <span className="text-[13px] font-medium text-primary ml-1">{label}</span>
    </div>
  );
}
