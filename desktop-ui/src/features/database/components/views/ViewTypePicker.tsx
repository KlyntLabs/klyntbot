import type { ViewType } from "@shared/types";
import { VIEW_TYPE_LABELS, VIEW_TYPES, ViewTypeIcon } from "./ViewTypeIcon";

interface ViewTypePickerProps {
  onSelect: (type: ViewType) => void;
}

export function ViewTypePicker({ onSelect }: ViewTypePickerProps) {
  return (
    <div className="p-2">
      <div className="px-1 pb-1.5 text-[11px] font-medium text-foreground/55">Add a view</div>
      <div className="grid grid-cols-3 gap-1">
        {VIEW_TYPES.map((type) => (
          <button
            key={type}
            type="button"
            onClick={() => onSelect(type)}
            className="flex flex-col items-center gap-1 rounded-md p-2 text-foreground/80 hover:bg-accent hover:text-foreground transition-colors"
          >
            <ViewTypeIcon type={type} className="h-5 w-5" />
            <span className="text-[11px]">{VIEW_TYPE_LABELS[type]}</span>
          </button>
        ))}
      </div>
    </div>
  );
}
