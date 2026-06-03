import type { ComponentPropsWithoutRef, ReactNode } from "react";
import { cn } from "@/utils/cn";

type SettingsSectionProps = {
  title: ReactNode;
  subtitle?: ReactNode;
  className?: string;
  children: ReactNode;
};

export function SettingsSection({ title, subtitle, className, children }: SettingsSectionProps) {
  return (
    <section className={cn("mb-4", className)}>
      <div className="text-ui-lg font-semibold text-text-strong mb-1">{title}</div>
      {subtitle ? <div className="text-ui-sm text-text-subtle mb-4">{subtitle}</div> : null}
      {children}
    </section>
  );
}

type SettingsSubsectionProps = {
  title: ReactNode;
  subtitle?: ReactNode;
  className?: string;
};

export function SettingsSubsection({ title, subtitle, className }: SettingsSubsectionProps) {
  return (
    <div className={className}>
      <div className="mt-[18px] mb-1.5 text-ui-sm font-bold tracking-wide uppercase text-text-muted">
        {title}
      </div>
      {subtitle ? <div className="text-ui-sm text-text-subtle mb-3">{subtitle}</div> : null}
    </div>
  );
}

type SettingsToggleRowProps = {
  title: ReactNode;
  subtitle?: ReactNode;
  className?: string;
  children: ReactNode;
};

export function SettingsToggleRow({
  title,
  subtitle,
  className,
  children,
}: SettingsToggleRowProps) {
  return (
    <div className={cn("flex items-center justify-between gap-4", className)}>
      <div>
        <div className="text-ui-sm font-semibold text-text-strong">{title}</div>
        {subtitle ? <div className="text-ui-xs text-text-subtle">{subtitle}</div> : null}
      </div>
      {children}
    </div>
  );
}

type SettingsToggleSwitchProps = Omit<
  ComponentPropsWithoutRef<"button">,
  "type" | "children" | "className" | "aria-pressed"
> & {
  pressed: boolean;
  className?: string;
};

export function SettingsToggleSwitch({ pressed, className, ...props }: SettingsToggleSwitchProps) {
  return (
    <button
      type="button"
      className={cn(
        "relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent",
        "transition-colors duration-ui-fast ease-ui-out",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-border-accent focus-visible:ring-offset-2",
        pressed ? "bg-border-accent" : "bg-surface-control",
        className,
      )}
      aria-pressed={pressed}
      {...props}
    >
      <span
        className={cn(
          "pointer-events-none inline-block h-4 w-4 rounded-full bg-white shadow ring-0",
          "transition duration-ui-fast ease-ui-out",
          pressed ? "translate-x-4" : "translate-x-0",
        )}
      />
    </button>
  );
}

/* ── Form primitives ─────────────────────────────────────────────────────── */

type SettingsFieldProps = {
  className?: string;
  children: ReactNode;
};

export function SettingsField({ className, children }: SettingsFieldProps) {
  return <div className={cn("flex flex-col gap-2.5 mb-4.5", className)}>{children}</div>;
}

type SettingsFieldLabelProps = ComponentPropsWithoutRef<"label"> & {
  children: ReactNode;
};

export function SettingsFieldLabel({ className, children, ...props }: SettingsFieldLabelProps) {
  return (
    <label className={cn("text-ui-sm font-semibold text-text-strong", className)} {...props}>
      {children}
    </label>
  );
}

type SettingsInputProps = ComponentPropsWithoutRef<"input"> & {
  compact?: boolean;
};

export function SettingsInput({ className, compact, ...props }: SettingsInputProps) {
  return (
    <input
      className={cn(
        "bg-surface-control border border-border-muted rounded-lg px-3 py-2 text-ui-sm text-text-primary",
        "outline-none focus-visible:border-border-strong focus-visible:ring-2 focus-visible:ring-border-accent",
        "placeholder:text-text-faint",
        compact && "py-1.5 px-2.5",
        className,
      )}
      {...props}
    />
  );
}

type SettingsSelectProps = ComponentPropsWithoutRef<"select">;

export function SettingsSelect({ className, children, ...props }: SettingsSelectProps) {
  return (
    <select
      className={cn(
        "bg-surface-control border border-border-muted rounded-lg px-3 py-2 pr-8 text-ui-sm text-text-primary",
        "outline-none focus-visible:border-border-strong focus-visible:ring-2 focus-visible:ring-border-accent",
        "appearance-none cursor-pointer",
        className,
      )}
      style={{
        backgroundImage: `linear-gradient(45deg,transparent 50%,var(--select-caret) 50%),linear-gradient(135deg,var(--select-caret) 50%,transparent 50%)`,
        backgroundPosition: `calc(100% - 10px) calc(50% + 1px), calc(100% - 6px) calc(50% + 1px)`,
        backgroundSize: `5px 5px, 5px 5px`,
        backgroundRepeat: `no-repeat`,
      }}
      {...props}
    >
      {children}
    </select>
  );
}

type SettingsHelpTextProps = {
  error?: boolean;
  className?: string;
  children: ReactNode;
};

export function SettingsHelpText({ error, className, children }: SettingsHelpTextProps) {
  return (
    <div className={cn("text-ui-xs", error ? "text-status-error" : "text-text-subtle", className)}>
      {children}
    </div>
  );
}

type SettingsFieldRowProps = {
  className?: string;
  children: ReactNode;
};

export function SettingsFieldRow({ className, children }: SettingsFieldRowProps) {
  return <div className={cn("flex gap-2 items-center flex-wrap", className)}>{children}</div>;
}
