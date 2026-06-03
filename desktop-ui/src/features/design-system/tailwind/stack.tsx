import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "@/utils/cn";

/* ═══════════════════════════════════════════════════════════════════════════
   Stack — Vertical flex layout primitive
   ══════════════════════════════════════════════════════════════════════════ */

export interface StackProps extends Omit<HTMLAttributes<HTMLDivElement>, "className"> {
  className?: string;
  children: ReactNode;
  gap?: "0" | "0.5" | "1" | "2" | "3" | "4" | "5" | "6" | "8" | "10" | "12";
  align?: "start" | "center" | "end" | "stretch";
  justify?: "start" | "center" | "end" | "between" | "around" | "evenly";
  wrap?: boolean;
}

const gapMap = {
  "0": "gap-0",
  "0.5": "gap-0.5",
  "1": "gap-1",
  "2": "gap-2",
  "3": "gap-3",
  "4": "gap-4",
  "5": "gap-5",
  "6": "gap-6",
  "8": "gap-8",
  "10": "gap-10",
  "12": "gap-12",
};

const alignMap = {
  start: "items-start",
  center: "items-center",
  end: "items-end",
  stretch: "items-stretch",
};

const justifyMap = {
  start: "justify-start",
  center: "justify-center",
  end: "justify-end",
  between: "justify-between",
  around: "justify-around",
  evenly: "justify-evenly",
};

export function Stack({
  className,
  children,
  gap = "2",
  align = "stretch",
  justify = "start",
  wrap = false,
  ...props
}: StackProps) {
  return (
    <div
      className={cn(
        "flex flex-col min-w-0",
        gapMap[gap],
        alignMap[align],
        justifyMap[justify],
        wrap && "flex-wrap",
        className,
      )}
      {...props}
    >
      {children}
    </div>
  );
}

/* ═══════════════════════════════════════════════════════════════════════════
   HStack — Horizontal flex layout primitive
   ══════════════════════════════════════════════════════════════════════════ */

export function HStack({
  className,
  children,
  gap = "2",
  align = "center",
  justify = "start",
  wrap = false,
  ...props
}: StackProps) {
  return (
    <div
      className={cn(
        "flex flex-row min-w-0",
        gapMap[gap],
        alignMap[align],
        justifyMap[justify],
        wrap && "flex-wrap",
        className,
      )}
      {...props}
    >
      {children}
    </div>
  );
}

/* ═══════════════════════════════════════════════════════════════════════════
   VStack — Alias for Stack (explicit vertical)
   ══════════════════════════════════════════════════════════════════════════ */

export function VStack(props: StackProps) {
  return <Stack {...props} />;
}
