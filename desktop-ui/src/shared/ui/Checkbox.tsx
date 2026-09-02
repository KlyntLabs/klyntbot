import * as CheckboxPrimitive from "@radix-ui/react-checkbox";
import { cn } from "@shared/lib/utils";
import { Check } from "lucide-react";

export interface CheckboxProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  className?: string;
  disabled?: boolean;
}

export function Checkbox({ checked, onCheckedChange, className, disabled }: CheckboxProps) {
  return (
    <CheckboxPrimitive.Root
      checked={checked}
      onCheckedChange={onCheckedChange}
      disabled={disabled}
      className={cn(
        "size-4 rounded border border-separator flex items-center justify-center transition-colors",
        "data-[state=checked]:bg-brand data-[state=checked]:border-brand",
        "disabled:opacity-50 disabled:pointer-events-none",
        className,
      )}
    >
      <CheckboxPrimitive.Indicator>
        <Check className="size-3 text-brand-foreground" strokeWidth={2} />
      </CheckboxPrimitive.Indicator>
    </CheckboxPrimitive.Root>
  );
}
