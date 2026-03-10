import { useQuery } from "@shared/hooks/useQuery";
import { TZ_OFFSET_MINS } from "@shared/lib/dates";
import type { ContextTimelineBlock } from "@shared/types";

export function useContextTimeline(date: string | null, tzOffsetMins?: number) {
  return useQuery<ContextTimelineBlock[]>(
    "get_context_timeline",
    date ? { date, tzOffsetMins: tzOffsetMins ?? TZ_OFFSET_MINS } : null,
    [],
  );
}
