import { cn } from "../../lib/cn";
import type { SkeletonProps } from "./Skeleton.types";

export function Skeleton({ className }: SkeletonProps) {
  return <div className={cn("animate-pulse rounded-control bg-control-hover", className)} />;
}
