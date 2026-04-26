// Plain replacement for the .bak's Radix-based Checkbox. Drives the same
// onCheckedChange API the FocusControl + tray callsites already use.
interface Props {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean;
  className?: string;
}

export function Checkbox({ checked, onCheckedChange, disabled, className }: Props) {
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onCheckedChange(!checked)}
      className={`tray-checkbox${checked ? " is-checked" : ""}${className ? ` ${className}` : ""}`}
    >
      {checked && (
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <path
            d="M3 8.5l3.5 3.5L13 5"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      )}
    </button>
  );
}
