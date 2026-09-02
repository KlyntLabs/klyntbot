import type { LucideIcon } from "lucide-react";
import { GitMerge, Users } from "lucide-react";

export type VoiceMode = "multi" | "synthesized";

interface VoiceToggleProps {
  mode: VoiceMode;
  onChange: (mode: VoiceMode) => void;
}

const MODES: { value: VoiceMode; icon: LucideIcon; label: string }[] = [
  { value: "multi", icon: Users, label: "Multi" },
  { value: "synthesized", icon: GitMerge, label: "Merged" },
];

export function VoiceToggle({ mode, onChange }: VoiceToggleProps) {
  return (
    <div className="flex items-center gap-0.5 rounded-md bg-white/[0.04] p-0.5">
      {MODES.map(({ value, icon: Icon, label }) => (
        <button
          key={value}
          type="button"
          onClick={() => onChange(value)}
          className={`flex items-center gap-1 text-ui-xs px-2 py-1 rounded transition-colors ${
            mode === value
              ? "bg-purple/20 text-purple-300"
              : "text-fg-secondary hover:text-fg"
          }`}
        >
          <Icon size={10} />
          {label}
        </button>
      ))}
    </div>
  );
}
