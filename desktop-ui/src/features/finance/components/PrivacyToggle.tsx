import { Eye, EyeOff } from "lucide-react";

export function PrivacyToggle({ hidden, onToggle }: { hidden: boolean; onToggle: () => void }) {
  const Icon = hidden ? EyeOff : Eye;
  return (
    <button
      type="button"
      onClick={onToggle}
      aria-label={hidden ? "Show sensitive amounts" : "Hide sensitive amounts"}
      className={`ml-2 p-2 rounded-lg transition-colors ${
        hidden
          ? "text-primary bg-white/[0.08]"
          : "text-muted hover:text-secondary hover:bg-white/[0.06]"
      }`}
    >
      <Icon className="w-3.5 h-3.5" strokeWidth={1.5} />
    </button>
  );
}
