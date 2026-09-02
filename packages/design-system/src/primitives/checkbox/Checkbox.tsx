import * as CheckboxPrimitive from "@radix-ui/react-checkbox";
import { cn } from "../../lib/cn";
import type { CheckboxProps } from "./Checkbox.types";

function CheckIcon({ className }: { className?: string }) {
  return (
    <svg
      className={className}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <polyline points="20 6 9 17 4 12" />
    </svg>
  );
}

export function Checkbox({ checked, onCheckedChange, className, disabled }: CheckboxProps) {
  return (
    <CheckboxPrimitive.Root
      checked={checked}
      onCheckedChange={(value) => onCheckedChange(value === true)}
      disabled={disabled}
      className={cn(
        "size-4 rounded border border-separator flex items-center justify-center transition-colors",
        "data-[state=checked]:bg-brand data-[state=checked]:border-brand",
        "disabled:opacity-50 disabled:pointer-events-none",
        className,
      )}
    >
      <CheckboxPrimitive.Indicator>
        <CheckIcon className="size-3 text-brand-foreground" />
      </CheckboxPrimitive.Indicator>
    </CheckboxPrimitive.Root>
  );
}
