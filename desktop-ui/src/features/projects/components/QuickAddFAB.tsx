// desktop-ui/src/features/projects/components/QuickAddFAB.tsx

import { useClickOutside } from "@shared/hooks/useClickOutside";
import { ChevronDown, Plus } from "lucide-react";
import { useRef, useState } from "react";

interface QuickAddFABProps {
  onAddTask: () => void;
  onAddNote: () => void;
  onAddObjective: () => void;
}

export function QuickAddFAB({ onAddTask, onAddNote, onAddObjective }: QuickAddFABProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useClickOutside(ref, () => setOpen(false), open);

  return (
    <div ref={ref} className="fixed bottom-5 right-6 z-50 flex items-center gap-px">
      <button
        type="button"
        onClick={onAddTask}
        className="flex items-center gap-1.5 px-4 py-2.5 rounded-l-lg bg-brand text-white text-xs font-medium hover:bg-brand/90 transition-colors"
      >
        <Plus className="size-3.5" /> Add Task
      </button>
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="px-2 py-2.5 rounded-r-lg bg-brand text-white hover:bg-brand/90 transition-colors border-l border-white/20"
      >
        <ChevronDown className="size-3.5" />
      </button>
      {open && (
        <div className="absolute bottom-full right-0 mb-2 glass-dropdown rounded-lg py-1 min-w-[160px]">
          <button
            type="button"
            onClick={() => {
              onAddTask();
              setOpen(false);
            }}
            className="w-full px-3 py-2 text-left text-xs text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
          >
            New Task
          </button>
          <button
            type="button"
            onClick={() => {
              onAddNote();
              setOpen(false);
            }}
            className="w-full px-3 py-2 text-left text-xs text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
          >
            New Note
          </button>
          <button
            type="button"
            onClick={() => {
              onAddObjective();
              setOpen(false);
            }}
            className="w-full px-3 py-2 text-left text-xs text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
          >
            New Objective
          </button>
        </div>
      )}
    </div>
  );
}
