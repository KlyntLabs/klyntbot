import { useContextTimeline } from "@features/work-contexts";
import { Tooltip } from "@shared/ui/Tooltip";
import { useMemo } from "react";

interface Props {
  date: string;
}

/**
 * A thin (4px) horizontal ribbon above the timeline showing context colors per hour.
 * Each hour block is colored based on the dominant work context active during that hour.
 */
export function ContextRibbon({ date }: Props) {
  const { data: blocks } = useContextTimeline(date);

  // Group blocks into 24 hourly buckets, pick dominant context color per hour
  const hourColors = useMemo(() => {
    const hours: (string | null)[] = Array.from({ length: 24 }, () => null);
    if (!blocks?.length) return hours;

    for (const block of blocks) {
      if (block.isIdle || !block.contextColor) continue;
      const start = new Date(block.startTime);
      const hour = start.getHours();
      if (hour >= 0 && hour < 24) {
        // Take the first (most events) context color for each hour
        if (!hours[hour]) {
          hours[hour] = block.contextColor;
        }
      }
    }
    return hours;
  }, [blocks]);

  const hasAny = hourColors.some(Boolean);
  if (!hasAny) return null;

  return (
    <div className="flex h-1 gap-px mx-12 mb-0.5 shrink-0">
      {hourColors.map((color, i) => (
        <Tooltip
          // biome-ignore lint/suspicious/noArrayIndexKey: fixed 24-hour slots, index is the hour
          key={i}
          content={`${i.toString().padStart(2, "0")}:00`}
        >
          <div
            className="flex-1 rounded-full transition-colors"
            style={{
              backgroundColor: color ?? "transparent",
              opacity: color ? 0.7 : 0,
            }}
          />
        </Tooltip>
      ))}
    </div>
  );
}
