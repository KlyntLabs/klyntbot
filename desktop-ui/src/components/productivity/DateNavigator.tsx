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
        className="w-7 h-7 rounded-md bg-surface-base flex items-center justify-center text-muted hover:text-secondary hover:bg-surface-raised transition-colors"
      >
        <ChevronLeft className="w-4 h-4" strokeWidth={1.5} />
      </button>
      <button
        type="button"
        onClick={onToday}
        className="w-7 h-7 rounded-md bg-surface-base flex items-center justify-center text-muted hover:text-secondary hover:bg-surface-raised transition-colors"
      >
        <Calendar className="w-3.5 h-3.5" strokeWidth={1.5} />
      </button>
      <button
        type="button"
        onClick={onNext}
        className="w-7 h-7 rounded-md bg-surface-base flex items-center justify-center text-muted hover:text-secondary hover:bg-surface-raised transition-colors"
      >
        <ChevronRight className="w-4 h-4" strokeWidth={1.5} />
      </button>
      <span className="text-[13px] font-medium text-primary ml-1">{label}</span>
    </div>
  );
}
