import { Eye, EyeOff } from "lucide-react";

interface TransparencyToggleProps {
  enabled: boolean;
  onToggle: () => void;
}

export function TransparencyToggle({ enabled, onToggle }: TransparencyToggleProps) {
  const Icon = enabled ? Eye : EyeOff;

  return (
    <button
      type="button"
      onClick={onToggle}
      aria-label={enabled ? "Hide transparency data" : "Show transparency data"}
      className={`size-8 flex items-center justify-center rounded-lg transition-colors ${
        enabled
          ? "bg-brand/10 text-brand hover:bg-brand/20"
          : "text-fg-secondary hover:bg-control-hover hover:text-fg"
      }`}
      title={enabled ? "Hide transparency data" : "Show transparency data"}
    >
      <Icon className="size-4" strokeWidth={1.5} />
    </button>
  );
}
