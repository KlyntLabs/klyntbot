import { MarkdownContent } from "@features/chat/components/MarkdownContent";
import { useInsightChat } from "@features/notes/hooks/useInsightChat";
import { FilePlus } from "lucide-react";
import type { TabStatus } from "../../hooks/useInsightReview";
import { InsightChatInput } from "./InsightChatInput";

interface GapAnalysisTabProps {
  status: TabStatus;
  content: string;
  noteId: string | null;
  squadId?: string | null;
  onCreateNote?: (title: string, body: string) => void;
}

interface Gap {
  topic: string;
  description: string;
  suggestedTitle: string;
}

function parseGaps(content: string): { markdown: string; gaps: Gap[] } {
  // Try to find a JSON block at the end
  const jsonMatch = content.match(/```json\s*\n([\s\S]*?)\n```\s*$/);
  if (!jsonMatch) return { markdown: content, gaps: [] };

  try {
    const gaps = JSON.parse(jsonMatch[1]) as Gap[];
    const markdown = content.slice(0, jsonMatch.index).trim();
    return { markdown, gaps };
  } catch {
    return { markdown: content, gaps: [] };
  }
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

export function GapAnalysisTab({
  status,
  content,
  noteId,
  squadId,
  onCreateNote,
}: GapAnalysisTabProps) {
  const chat = useInsightChat(noteId, "gaps", status === "done", squadId, content);
  if (status === "idle") {
    return (
      <p className="text-[11px] text-dim italic">Start an insight review to see gap analysis</p>
    );
  }

  if (status === "loading") {
    return <SkeletonLoader />;
  }

  if (status === "error") {
    return (
      <p className="text-[11px] text-destructive">
        Failed to generate gap analysis. Try regenerating.
      </p>
    );
  }

  const { markdown, gaps } = parseGaps(content);

  return (
    <div>
      <div className="space-y-4">
        <div className="text-xs text-muted-foreground leading-relaxed">
          <MarkdownContent content={markdown} />
        </div>

        {gaps.length > 0 && (
          <div className="space-y-2 pt-2 border-t border-border">
            <div className="text-2xs font-medium text-dim uppercase tracking-wider">
              Knowledge Gaps — Deep Dive
            </div>
            {gaps.map((gap) => (
              <button
                key={gap.topic}
                type="button"
                onClick={() => onCreateNote?.(gap.suggestedTitle, gap.description)}
                className="w-full flex items-start gap-2 p-2 rounded-lg bg-card hover:bg-accent transition-colors text-left group"
              >
                <FilePlus size={12} className="text-dim group-hover:text-brand mt-0.5 shrink-0" />
                <div>
                  <div className="text-[11px] text-muted-foreground group-hover:text-foreground">
                    {gap.topic}
                  </div>
                  <div className="text-2xs text-dim mt-0.5 line-clamp-2">{gap.description}</div>
                </div>
              </button>
            ))}
          </div>
        )}
      </div>
      {status === "done" && (
        <InsightChatInput {...chat} placeholder="Ask about these knowledge gaps..." />
      )}
    </div>
  );
}
