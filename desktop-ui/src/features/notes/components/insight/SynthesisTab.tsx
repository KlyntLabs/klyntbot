import { MarkdownContent } from "@features/chat/components/MarkdownContent";
import type { TabStatus } from "../../hooks/useInsightReview";

interface SynthesisTabProps {
  status: TabStatus;
  content: string;
}

function SkeletonLoader() {
  return (
    <div className="space-y-3 animate-pulse">
      <div className="h-3 bg-white/[0.04] rounded w-3/4" />
      <div className="h-3 bg-white/[0.04] rounded w-full" />
      <div className="h-3 bg-white/[0.04] rounded w-5/6" />
    </div>
  );
}

export function SynthesisTab({ status, content }: SynthesisTabProps) {
  if (status === "idle") {
    return <p className="text-[11px] text-dim italic">Start an insight review to see synthesis</p>;
  }

  if (status === "loading") {
    return <SkeletonLoader />;
  }

  if (status === "error") {
    return (
      <p className="text-[11px] text-red-400">Failed to generate synthesis. Try regenerating.</p>
    );
  }

  // streaming or done
  return (
    <div className="text-[12px] text-secondary leading-relaxed">
      <MarkdownContent content={content} />
      {status === "streaming" && (
        <span className="inline-block w-1.5 h-3.5 bg-purple-400 animate-pulse ml-0.5 align-text-bottom rounded-sm" />
      )}
    </div>
  );
}
