import type { ImgHTMLAttributes } from "react";
import { cn } from "@/utils/cn";

/* ═══════════════════════════════════════════════════════════════════════════
   Avatar — User/image avatar primitive
   ══════════════════════════════════════════════════════════════════════════ */

export interface AvatarProps extends Omit<ImgHTMLAttributes<HTMLImageElement>, "className"> {
  className?: string;
  size?: "xs" | "sm" | "md" | "lg" | "xl";
  fallback?: string;
  src?: string;
  alt: string;
}

const sizeMap = {
  xs: "size-5 text-ui-2xs",
  sm: "size-6 text-ui-xs",
  md: "size-8 text-ui-sm",
  lg: "size-10 text-ui-md",
  xl: "size-12 text-ui-lg",
};

export function Avatar({ className, size = "md", fallback, src, alt, ...props }: AvatarProps) {
  const initials = fallback || alt.slice(0, 2).toUpperCase();

  return (
    <div
      className={cn(
        "relative inline-flex items-center justify-center rounded-full",
        "bg-surface-card-strong text-text-strong font-semibold",
        "overflow-hidden shrink-0",
        sizeMap[size],
        className,
      )}
      title={alt}
    >
      {src ? (
        <img
          src={src}
          alt={alt}
          className="absolute inset-0 size-full object-cover"
          {...props}
        />
      ) : (
        <span className="relative z-10">{initials}</span>
      )}
    </div>
  );
}
