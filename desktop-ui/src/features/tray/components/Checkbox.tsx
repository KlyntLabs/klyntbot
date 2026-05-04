// Plain replacement for the .bak's Radix-based Checkbox. Drives the same
// onCheckedChange API the FocusControl + tray callsites already use.
interface Props {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean;
  className?: string;
  id?: string;
}

export function Checkbox({ checked, onCheckedChange, disabled, className, id }: Props) {
  return (
    <span
      className={`tray-checkbox${checked ? " is-checked" : ""}${className ? ` ${className}` : ""}`}
    >
      <input
        type="checkbox"
        id={id}
        checked={checked}
        onChange={() => onCheckedChange(!checked)}
        disabled={disabled}
        className="tray-checkbox-native"
      />
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
    </span>
  );
}
