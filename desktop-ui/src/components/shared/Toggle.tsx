interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  /** Compact size for dense layouts like tables */
  size?: "default" | "sm";
}

export function Toggle({ checked, onChange, disabled, size = "default" }: ToggleProps) {
  const isSmall = size === "sm";
  return (
    <button
      type="button"
      onClick={() => onChange(!checked)}
      disabled={disabled}
      className={`relative rounded-full transition-colors disabled:opacity-50 ${
        isSmall ? "w-7 h-4" : "w-9 h-5"
      } ${checked ? "bg-brand" : "bg-white/[0.1]"}`}
    >
      <span
        className={`absolute top-0.5 left-0.5 rounded-full bg-white transition-transform ${
          isSmall ? "w-3 h-3" : "w-4 h-4"
        } ${checked ? (isSmall ? "translate-x-3" : "translate-x-4") : ""}`}
      />
    </button>
  );
}
