import type { ViewType } from "@shared/types";
import { Skeleton } from "@shared/ui/Skeleton";

interface Props {
  viewType: ViewType;
}

const COUNT = 8;

export function ViewLoadingSkeleton({ viewType }: Props) {
  switch (viewType) {
    case "table":
      return (
        <div className="space-y-1 p-4">
          {Array.from({ length: COUNT }).map((_, i) => (
            <Skeleton key={i} className="h-9 w-full" />
          ))}
        </div>
      );
    case "list":
    case "feed":
      return (
        <div className="space-y-2 p-4">
          {Array.from({ length: COUNT }).map((_, i) => (
            <Skeleton key={i} className="h-12 w-full" />
          ))}
        </div>
      );
    case "gallery":
      return (
        <div className="grid grid-cols-2 gap-4 p-4 sm:grid-cols-3 lg:grid-cols-4">
          {Array.from({ length: COUNT }).map((_, i) => (
            <Skeleton key={i} className="h-32 w-full" />
          ))}
        </div>
      );
    case "board":
      return (
        <div className="flex gap-4 p-4">
          {Array.from({ length: 4 }).map((_, c) => (
            <div key={c} className="flex w-64 shrink-0 flex-col gap-2">
              <Skeleton className="h-6 w-24" />
              {Array.from({ length: 3 }).map((_, i) => (
                <Skeleton key={i} className="h-20 w-full" />
              ))}
            </div>
          ))}
        </div>
      );
    case "calendar":
    case "timeline":
      return <Skeleton className="m-4 h-[480px] w-[calc(100%-2rem)]" />;
    case "chart":
      return <Skeleton className="m-4 h-[400px] w-[calc(100%-2rem)]" />;
    default:
      return null;
  }
}
