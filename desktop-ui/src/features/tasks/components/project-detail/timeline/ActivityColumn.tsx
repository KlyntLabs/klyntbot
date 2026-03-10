import { useQuery } from "@shared/hooks/useQuery";
import { todayISO } from "@shared/lib/dates";
import type { ContextTimelineBlock } from "@shared/types";

interface ActivityColumnProps {
  projectId: string;
}

export function ActivityColumn({ projectId }: ActivityColumnProps) {
  const { data: blocks } = useQuery<ContextTimelineBlock[]>(
    "get_context_timeline",
    { date: todayISO(), projectId },
    [],
  );

  if (blocks.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-[11px] text-dim font-light">
        No activity today
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-1 p-2">
      {blocks.map((block) => (
        <div
          key={`${block.startTime}-${block.title}`}
          className="rounded-md px-2.5 py-2 text-left"
          style={{ backgroundColor: `${block.color ?? "#6B7280"}20` }}
        >
          <p className="text-[11px] font-light text-secondary truncate">{block.title}</p>
          <p className="text-[9px] text-dim">
            {block.contextType} &middot; {block.durationMins}m
          </p>
        </div>
      ))}
    </div>
  );
}
