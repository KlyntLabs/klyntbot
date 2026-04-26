import { cn } from "@shared/lib/utils";

export interface SkeletonProps {
  className?: string;
}

export function Skeleton({ className }: SkeletonProps) {
  return <div className={cn("animate-pulse rounded-lg bg-accent", className)} />;
}
