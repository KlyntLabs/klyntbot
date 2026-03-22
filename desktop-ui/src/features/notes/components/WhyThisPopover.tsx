import * as PopoverPrimitive from "@radix-ui/react-popover";
import { cn } from "@shared/lib/cn";
import { type ReactNode, useEffect, useRef, useState } from "react";

interface WhyThisPopoverProps {
  sourceContext?: string | null;
  domain: string;
  children: ReactNode;
}

export function WhyThisPopover({ sourceContext, domain, children }: WhyThisPopoverProps) {
  const [open, setOpen] = useState(false);
  const timeoutRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  const handleEnter = () => {
    clearTimeout(timeoutRef.current);
    timeoutRef.current = setTimeout(() => setOpen(true), 300);
  };

  const handleLeave = () => {
    clearTimeout(timeoutRef.current);
    timeoutRef.current = setTimeout(() => setOpen(false), 150);
  };

  useEffect(() => () => clearTimeout(timeoutRef.current), []);

  return (
    <PopoverPrimitive.Root open={open} onOpenChange={setOpen}>
      <PopoverPrimitive.Trigger asChild>
        <div onMouseEnter={handleEnter} onMouseLeave={handleLeave}>
          {children}
        </div>
      </PopoverPrimitive.Trigger>

      <PopoverPrimitive.Portal>
        <PopoverPrimitive.Content
          side="top"
          align="start"
          sideOffset={4}
          collisionPadding={8}
          onMouseEnter={handleEnter}
          onMouseLeave={handleLeave}
          onOpenAutoFocus={(e) => e.preventDefault()}
          onCloseAutoFocus={(e) => e.preventDefault()}
          className={cn(
            "z-50 w-56 rounded-lg border border-border bg-popover p-2.5 shadow-lg outline-none",
            "data-[state=open]:animate-in data-[state=closed]:animate-out",
            "data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0",
            "data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95",
            "data-[side=top]:slide-in-from-bottom-2 data-[side=bottom]:slide-in-from-top-2",
          )}
        >
          <p className="text-[10px] text-muted mb-1">Why this was suggested</p>
          <p className="text-xs text-primary">{domain}</p>
          {sourceContext && (
            <p className="text-[10px] text-muted mt-1 line-clamp-3">
              &ldquo;{sourceContext}&rdquo;
            </p>
          )}
        </PopoverPrimitive.Content>
      </PopoverPrimitive.Portal>
    </PopoverPrimitive.Root>
  );
}
