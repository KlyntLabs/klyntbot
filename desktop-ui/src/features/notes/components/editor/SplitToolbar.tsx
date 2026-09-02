import { BookOpen, FileText, Languages, StickyNote } from "lucide-react";
import type { SplitMode } from "./SplitEditor";

type EditorMode = "single" | SplitMode;

interface SplitToolbarProps {
  currentMode: EditorMode;
  onModeChange: (mode: EditorMode) => void;
}

const modes: { key: EditorMode; icon: typeof FileText; label: string; shortLabel: string }[] = [
  { key: "single", icon: FileText, label: "Single pane", shortLabel: "Single" },
  { key: "translation", icon: Languages, label: "Translation mode", shortLabel: "Translate" },
  { key: "annotation", icon: StickyNote, label: "Annotation mode", shortLabel: "Annotate" },
  { key: "cornell", icon: BookOpen, label: "Cornell method", shortLabel: "Cornell" },
];

export function SplitToolbar({ currentMode, onModeChange }: SplitToolbarProps) {
  return (
    <div className="flex items-center gap-0.5 px-2 py-1">
      {modes.map((mode) => {
        const Icon = mode.icon;
        const isActive = currentMode === mode.key;
        return (
          <button
            key={mode.key}
            type="button"
            onClick={() => onModeChange(mode.key)}
            title={mode.label}
            className={`flex items-center gap-1 px-2 py-1 rounded-lg text-ui-xs transition-all ${
              isActive
                ? "bg-brand/15 text-brand"
                : "text-fg-secondary hover:text-fg hover:bg-control-hover"
            }`}
          >
            <Icon className="size-3.5" strokeWidth={1.5} />
            {mode.shortLabel}
          </button>
        );
      })}
    </div>
  );
}
