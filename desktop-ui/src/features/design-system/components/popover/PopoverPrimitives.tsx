import { type ComponentPropsWithoutRef, forwardRef, type ReactNode, type RefObject } from "react";
import { cn } from "@/utils/cn";

type PopoverSurfaceProps = ComponentPropsWithoutRef<"div"> & {
  children: ReactNode;
};

export const PopoverSurface = forwardRef<HTMLDivElement, PopoverSurfaceProps>(
  function PopoverSurface({ className, ...props }, ref) {
    return (
      <div
        ref={ref}
        className={cn(
          "bg-surface-popover border border-border-muted rounded-ui-lg shadow-ds-popover",
          "animate-[ds-popover-in_var(--ds-dur-fast)_var(--ds-ease-out)_both]",
          className,
        )}
        {...props}
      />
    );
  },
);

type PopoverMenuItemProps = Omit<ComponentPropsWithoutRef<"button">, "children"> & {
  children: ReactNode;
  icon?: ReactNode;
  active?: boolean;
};

export function PopoverMenuItem({
  className,
  icon,
  active = false,
  children,
  ...props
}: PopoverMenuItemProps) {
  return (
    <button
      type="button"
      className={cn(
        "w-full flex items-center justify-start gap-2 px-2 py-1.5 rounded-ui-md",
        "bg-transparent border-none text-text-muted text-ui-sm text-left cursor-pointer",
        "transition-colors duration-ui-fast ease-ui-out-soft",
        "hover:bg-surface-hover hover:text-text-stronger focus-visible:bg-surface-hover focus-visible:text-text-stronger",
        "disabled:opacity-[0.55] disabled:cursor-not-allowed",
        active && "bg-surface-hover text-text-stronger",
        className,
      )}
      {...props}
    >
      {icon ? (
        <span className="shrink-0 inline-flex items-center justify-center w-3.5 h-3.5" aria-hidden>
          {icon}
        </span>
      ) : null}
      <span className="min-w-0 flex-1 truncate">{children}</span>
    </button>
  );
}

type MenuTriggerProps = Omit<
  ComponentPropsWithoutRef<"button">,
  "aria-expanded" | "aria-haspopup"
> & {
  isOpen: boolean;
  popupRole?: "menu" | "dialog";
  activeClassName?: string;
  "data-tauri-drag-region"?: string;
};

export function MenuTrigger({
  isOpen,
  popupRole = "menu",
  className,
  activeClassName,
  "data-tauri-drag-region": dragRegion,
  ...props
}: MenuTriggerProps) {
  return (
    <button
      type="button"
      aria-haspopup={popupRole}
      aria-expanded={isOpen}
      className={cn(className, isOpen && activeClassName)}
      data-tauri-drag-region={dragRegion ?? "false"}
      {...props}
    />
  );
}

type SplitActionMenuProps = {
  containerRef?: RefObject<HTMLDivElement | null>;
  className?: string;
  buttonGroupClassName?: string;
  actionButton: ReactNode;
  isOpen: boolean;
  onToggle: () => void;
  toggleClassName?: string;
  toggleAriaLabel: string;
  toggleTitle?: string;
  toggleTooltip?: string;
  toggleTooltipPlacement?: "top" | "bottom";
  toggleTooltipAlign?: "start" | "end";
  toggleIcon: ReactNode;
  popoverClassName?: string;
  popoverRole?: "menu" | "dialog";
  children: ReactNode;
};

export function SplitActionMenu({
  containerRef,
  className,
  buttonGroupClassName,
  actionButton,
  isOpen,
  onToggle,
  toggleClassName,
  toggleAriaLabel,
  toggleTitle,
  toggleTooltip,
  toggleTooltipPlacement,
  toggleTooltipAlign,
  toggleIcon,
  popoverClassName,
  popoverRole = "menu",
  children,
}: SplitActionMenuProps) {
  return (
    <div className={className} ref={containerRef}>
      <div className={buttonGroupClassName}>
        {actionButton}
        <MenuTrigger
          isOpen={isOpen}
          popupRole={popoverRole}
          className={toggleClassName}
          onClick={onToggle}
          aria-label={toggleAriaLabel}
          title={toggleTitle}
          data-tooltip={toggleTooltip}
          data-tooltip-placement={toggleTooltipPlacement}
          data-tooltip-align={toggleTooltipAlign}
        >
          {toggleIcon}
        </MenuTrigger>
      </div>
      {isOpen && (
        <PopoverSurface className={popoverClassName} role={popoverRole}>
          {children}
        </PopoverSurface>
      )}
    </div>
  );
}
