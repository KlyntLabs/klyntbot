import { ipc } from "@shared/hooks/useIpc";
import { cn } from "@shared/lib/utils";
import { useCallback, useId, useRef, useState } from "react";

interface MemoryReferenceDetail {
  refType: string;
  title: string;
  subtitle: string;
  details: Record<string, string>;
}

interface MemoryReferenceProps {
  refType: string;
  refId: string;
}

/** In-memory cache so re-hovering the same reference doesn't re-fetch. */
const detailCache = new Map<string, MemoryReferenceDetail>();

const TYPE_LABELS: Record<string, string> = {
  fact: "fact",
  rule: "rule",
  episode: "episode",
};

export function MemoryReference({ refType, refId }: MemoryReferenceProps) {
  const [visible, setVisible] = useState(false);
  const [detail, setDetail] = useState<MemoryReferenceDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const tooltipId = useId();
  const hideTimerRef = useRef<ReturnType<typeof setTimeout>>();

  const fetchDetail = useCallback(async () => {
    const cacheKey = `${refType}:${refId}`;
    const cached = detailCache.get(cacheKey);
    if (cached) {
      setDetail(cached);
      return;
    }

    setLoading(true);
    try {
      const result = await ipc<MemoryReferenceDetail>("memory_reference_detail", {
        refType,
        refId,
      });
      detailCache.set(cacheKey, result);
      setDetail(result);
    } catch {
      setDetail({
        refType,
        title: "Failed to load",
        subtitle: `${refType}:${refId}`,
        details: {},
      });
    } finally {
      setLoading(false);
    }
  }, [refType, refId]);

  const handleMouseEnter = () => {
    clearTimeout(hideTimerRef.current);
    setVisible(true);
    if (!detail && !loading) {
      fetchDetail();
    }
  };

  const handleMouseLeave = () => {
    // Small delay so the user can move to the tooltip
    hideTimerRef.current = setTimeout(() => setVisible(false), 150);
  };

  const label = TYPE_LABELS[refType] ?? refType;

  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: tooltip wrapper uses mouse events for show/hide
    <span
      className="relative inline-block"
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
      onFocus={handleMouseEnter}
      onBlur={() => setVisible(false)}
      aria-describedby={visible ? tooltipId : undefined}
    >
      <span
        className={cn(
          "text-[11px] text-muted-foreground cursor-default",
          "border-b border-dotted border-muted-foreground/40",
          "hover:text-foreground hover:border-foreground/40 transition-colors",
        )}
      >
        ({label})
      </span>

      {visible && (
        <div
          id={tooltipId}
          role="tooltip"
          className={cn(
            "absolute z-50 bottom-full mb-1.5 left-1/2 -translate-x-1/2",
            "min-w-[220px] max-w-[320px] p-2.5 rounded-lg",
            "glass-panel border border-border shadow-lg",
            "animate-in fade-in-50 duration-100",
          )}
          onMouseEnter={() => clearTimeout(hideTimerRef.current)}
          onMouseLeave={handleMouseLeave}
        >
          {loading && !detail ? (
            <span className="text-[11px] text-dim">Loading...</span>
          ) : detail ? (
            <div className="space-y-1.5">
              <div className="text-xs font-medium text-foreground leading-snug">{detail.title}</div>
              <div className="text-[11px] text-muted-foreground">{detail.subtitle}</div>
              {Object.keys(detail.details).length > 0 && (
                <div className="border-t border-border pt-1.5 space-y-0.5">
                  {Object.entries(detail.details).map(([key, value]) => (
                    <div key={key} className="flex justify-between text-[11px]">
                      <span className="text-dim">{key}</span>
                      <span className="text-muted-foreground ml-2 text-right">{value}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          ) : null}
        </div>
      )}
    </span>
  );
}
