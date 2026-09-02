import { cn } from "@klyntbot/design-system";

export interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  size?: "default" | "sm";
  className?: string;
}

export function Toggle({ checked, onChange, disabled, size = "default", className }: ToggleProps) {
  const isSmall = size === "sm";
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      disabled={disabled}
      className={cn(
        "relative rounded-full transition-colors disabled:opacity-50",
        isSmall ? "w-7 h-4" : "w-9 h-5",
        checked ? "bg-brand" : "bg-control-active",
        className,
      )}
    >
      <span
        className={cn(
          "absolute top-0.5 left-0.5 rounded-full bg-brand-foreground transition-transform",
          isSmall ? "size-3" : "size-4",
          checked ? (isSmall ? "translate-x-3" : "translate-x-4") : "",
        )}
      />
    </button>
  );
}
