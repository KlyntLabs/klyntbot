import * as CheckboxPrimitive from "@radix-ui/react-checkbox";
import { Check } from "lucide-react";
import { cn } from "../../lib/utils";

interface CheckboxProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  className?: string;
}

export function Checkbox({ checked, onCheckedChange, className }: CheckboxProps) {
  return (
    <CheckboxPrimitive.Root
      checked={checked}
      onCheckedChange={onCheckedChange}
      className={cn(
        "h-4 w-4 rounded border border-muted/40 flex items-center justify-center transition-colors",
        "data-[state=checked]:bg-brand data-[state=checked]:border-brand",
        className,
      )}
    >
      <CheckboxPrimitive.Indicator>
        <Check className="h-3 w-3 text-white" strokeWidth={2} />
      </CheckboxPrimitive.Indicator>
    </CheckboxPrimitive.Root>
  );
}
