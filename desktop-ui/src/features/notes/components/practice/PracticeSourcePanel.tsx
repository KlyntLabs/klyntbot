import { useEffect, useRef } from "react";

interface SourceSegment {
  index: number;
  text: string;
  type: string;
  suggestedFocus: string;
}

interface PracticeSourcePanelProps {
  segments: SourceSegment[];
  currentIndex: number;
  completedIndices: Set<number>;
}

export function PracticeSourcePanel({
  segments,
  currentIndex,
  completedIndices,
}: PracticeSourcePanelProps) {
  const containerRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to keep current segment visible
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const el = container.querySelector(`[data-segment-index="${currentIndex}"]`);
    if (el) {
      el.scrollIntoView({ behavior: "smooth", block: "center" });
    }
  }, [currentIndex]);

  return (
    <div ref={containerRef} className="p-4 space-y-2 overflow-auto h-full">
      {segments.map((segment) => {
        const isCurrent = segment.index === currentIndex;
        const isCompleted = completedIndices.has(segment.index);

        let className = "text-sm leading-relaxed py-1 transition-colors duration-200";

        if (isCurrent) {
          className +=
            " bg-brand/10 border-l-2 border-brand pl-2 rounded-r text-primary font-medium";
        } else if (isCompleted) {
          className += " opacity-40 line-through pl-3";
        } else {
          className += " text-muted pl-3";
        }

        return (
          <p key={segment.index} data-segment-index={segment.index} className={className}>
            {segment.text}
          </p>
        );
      })}
    </div>
  );
}
