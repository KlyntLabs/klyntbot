import type { AriaRole, ComponentPropsWithoutRef, ReactNode } from "react";
import { cn } from "@/utils/cn";

type ToastViewportProps = Omit<ComponentPropsWithoutRef<"div">, "children" | "role"> & {
  children: ReactNode;
  className?: string;
  role?: AriaRole;
  ariaLive?: "off" | "polite" | "assertive";
};

export function ToastViewport({
  children,
  className,
  role,
  ariaLive,
  ...props
}: ToastViewportProps) {
  return (
    <div className={cn("grid gap-3", className)} role={role} aria-live={ariaLive} {...props}>
      {children}
    </div>
  );
}

type ToastCardProps = Omit<ComponentPropsWithoutRef<"div">, "children" | "role"> & {
  children: ReactNode;
  className?: string;
  role?: AriaRole;
};

export function ToastCard({ children, className, role, ...props }: ToastCardProps) {
  return (
    <div
      className={cn(
        "bg-ds-surface-overlay border border-ds-border-subtle shadow-ds-toast rounded-xl p-3 pointer-events-auto max-w-full",
        "animate-[ds-toast-in_var(--ds-toast-enter-duration,0.2s)_ease-out]",
        className,
      )}
      role={role}
      {...props}
    >
      {children}
    </div>
  );
}

type ToastTextProps = ComponentPropsWithoutRef<"div">;

export function ToastTitle({ className, ...props }: ToastTextProps) {
  return <div className={cn("text-ds-text-subtle", className)} {...props} />;
}

export function ToastBody({ className, ...props }: ToastTextProps) {
  return (
    <div
      className={cn("text-ds-toast-body overflow-wrap-anywhere break-words", className)}
      {...props}
    />
  );
}

type ToastSectionProps = ComponentPropsWithoutRef<"div">;

export function ToastHeader({ className, ...props }: ToastSectionProps) {
  return (
    <div className={cn("flex items-center justify-between gap-3", className)} {...props} />
  );
}

export function ToastActions({ className, ...props }: ToastSectionProps) {
  return (
    <div className={cn("flex gap-2 justify-end flex-wrap", className)} {...props} />
  );
}

type ToastErrorProps = ComponentPropsWithoutRef<"pre">;

export function ToastError({ className, ...props }: ToastErrorProps) {
  return (
    <pre
      className={cn(
        "font-code text-ui-xs text-text-muted whitespace-pre-wrap",
        "max-h-[120px] overflow-auto rounded-ui-md bg-surface-card-muted p-2 m-0",
        "overflow-wrap-anywhere break-words",
        className,
      )}
      {...props}
    />
  );
}
