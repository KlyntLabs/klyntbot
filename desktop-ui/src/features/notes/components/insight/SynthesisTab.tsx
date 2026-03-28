import { MarkdownContent } from "@features/chat/components/MarkdownContent";
import { useInsightChat } from "@features/notes/hooks/useInsightChat";
import type { TreePathRef } from "@shared/types/tree-path";
import type { TabStatus } from "../../hooks/useInsightReview";
import { InsightChatInput } from "./InsightChatInput";
import { StructureTreeView } from "./StructureTreeView";

interface SynthesisResult {
  treePaths?: TreePathRef[];
}

interface SynthesisTabProps {
  status: TabStatus;
  content: string;
  noteId: string | null;
  squadId?: string | null;
  synthesisResult?: SynthesisResult | null;
}

function SkeletonLoader() {
  return (
    <div className="space-y-3 animate-pulse">
      <div className="h-3 bg-card rounded w-3/4" />
      <div className="h-3 bg-card rounded w-full" />
      <div className="h-3 bg-card rounded w-5/6" />
    </div>
  );
}

export function SynthesisTab({
  status,
  content,
  noteId,
  squadId,
  synthesisResult,
}: SynthesisTabProps) {
  const chat = useInsightChat(noteId, "synthesis", status === "done", squadId, content);
  if (status === "idle") {
    return <p className="text-[11px] text-dim italic">Start an insight review to see synthesis</p>;
  }

  if (status === "loading") {
    return <SkeletonLoader />;
  }

  if (status === "error") {
    return (
      <p className="text-[11px] text-destructive">
        Failed to generate synthesis. Try regenerating.
      </p>
    );
  }

  // streaming with no content yet — show generating indicator
  if (status === "streaming" && !content) {
    return (
      <div className="space-y-3">
        <div className="flex items-center gap-2">
          <span className="inline-block w-1.5 h-3.5 bg-purple animate-pulse rounded-sm" />
          <span className="text-[11px] text-muted-foreground animate-pulse">
            Generating synthesis...
          </span>
        </div>
        <SkeletonLoader />
      </div>
    );
  }

  // streaming with content, or done
  return (
    <div>
      <div className="text-xs text-muted-foreground leading-relaxed">
        <MarkdownContent content={content} />
        {status === "streaming" && (
          <span className="inline-block w-1.5 h-3.5 bg-purple animate-pulse ml-0.5 align-text-bottom rounded-sm" />
        )}
      </div>
      {status === "done" && synthesisResult?.treePaths && synthesisResult.treePaths.length > 0 && (
        <StructureTreeView treePaths={synthesisResult.treePaths} />
      )}
      {status === "done" && (
        <InsightChatInput {...chat} placeholder="Ask about this synthesis..." />
      )}
    </div>
  );
}
