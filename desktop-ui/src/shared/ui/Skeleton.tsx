import { useReducedMotion } from "@shared/hooks/useReducedMotion";
import { cn } from "@shared/lib/utils";

export interface SkeletonProps {
  className?: string;
}

export function Skeleton({ className }: SkeletonProps) {
  const reduced = useReducedMotion();
  return <div className={cn(reduced ? "" : "animate-pulse", "rounded-lg bg-accent", className)} />;
}
