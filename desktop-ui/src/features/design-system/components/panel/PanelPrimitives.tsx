import ChevronRight from "lucide-react/dist/esm/icons/chevron-right";
import type { ComponentPropsWithoutRef, ReactNode } from "react";
import { cn } from "@/utils/cn";

type PanelFrameProps = {
  children: ReactNode;
  className?: string;
};

export function PanelFrame({ children, className }: PanelFrameProps) {
  return (
    <aside
      className={cn(
        "flex flex-col gap-2 bg-transparent p-3 pt-3 pb-0 min-h-0 flex-1 select-none",
        "[-webkit-app-region:no-drag]",
        className,
      )}
    >
      {children}
    </aside>
  );
}

type PanelHeaderProps = {
  children: ReactNode;
  className?: string;
};

export function PanelHeader({ children, className }: PanelHeaderProps) {
  return (
    <div
      className={cn(
        "flex items-center justify-between min-h-[26px] text-ds-panel-header-text",
        className,
      )}
    >
      {children}
    </div>
  );
}

type PanelMetaProps = {
  children: ReactNode;
  className?: string;
};

export function PanelMeta({ children, className }: PanelMetaProps) {
  return (
    <div className={cn("flex items-center gap-1.5 text-ui-xs text-text-faint", className)}>
      {children}
    </div>
  );
}

type PanelSearchFieldProps = Omit<
  ComponentPropsWithoutRef<"input">,
  "className" | "type"
> & {
  className?: string;
  inputClassName?: string;
  icon?: ReactNode;
  trailing?: ReactNode;
};

export function PanelSearchField({
  className,
  inputClassName,
  icon,
  trailing,
  ...props
}: PanelSearchFieldProps) {
  return (
    <div
      className={cn(
        "flex items-center gap-2 px-2 py-1.5 rounded-ui-lg",
        "bg-surface-raised border border-border-subtle text-text-faint",
        "focus-within:border-border-strong focus-within:text-text-emphasis",
        "transition-colors duration-ui-fast",
        className,
      )}
    >
      {icon ? (
        <span className="shrink-0 inline-flex items-center justify-center w-3.5 h-3.5" aria-hidden>
          {icon}
        </span>
      ) : null}
      <input
        type="search"
        className={cn(
          "flex-1 min-w-0 bg-transparent text-text-primary placeholder:text-text-faint outline-none",
          "text-ui-sm leading-none",
          inputClassName,
        )}
        {...props}
      />
      {trailing}
    </div>
  );
}

type PanelNavListProps = {
  children: ReactNode;
  className?: string;
};

export function PanelNavList({ children, className }: PanelNavListProps) {
  return <div className={cn("flex flex-col gap-1.5", className)}>{children}</div>;
}

type PanelNavItemProps = Omit<ComponentPropsWithoutRef<"button">, "children"> & {
  children: ReactNode;
  icon?: ReactNode;
  active?: boolean;
  showDisclosure?: boolean;
};

export function PanelNavItem({
  className,
  icon,
  active = false,
  showDisclosure = false,
  children,
  ...props
}: PanelNavItemProps) {
  return (
    <button
      type="button"
      className={cn(
        "w-full flex items-center justify-between gap-2.5 px-2.5 py-2 rounded-ui-lg",
        "text-ui-sm font-semibold text-left cursor-pointer select-none",
        "border border-transparent bg-transparent text-text-muted",
        "transition-colors duration-ui-fast",
        "hover:bg-surface-card hover:border-border-strong hover:text-text-strong",
        "focus-visible:bg-surface-card focus-visible:border-border-strong focus-visible:text-text-strong",
        active && "bg-surface-card border-border-strong text-text-strong",
        className,
      )}
      {...props}
    >
      <span className="min-w-0 flex items-center gap-2.5 flex-1">
        {icon ? (
          <span className="shrink-0 inline-flex items-center justify-center w-4 h-4" aria-hidden>
            {icon}
          </span>
        ) : null}
        <span className="min-w-0 flex-1 truncate">{children}</span>
      </span>
      {showDisclosure ? (
        <span
          className="shrink-0 inline-flex items-center justify-center w-4 h-4 text-text-faint"
          aria-hidden
        >
          <ChevronRight />
        </span>
      ) : null}
    </button>
  );
}
