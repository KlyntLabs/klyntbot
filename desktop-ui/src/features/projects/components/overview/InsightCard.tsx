import { ipc } from "@shared/hooks/useIpc";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router";
import { useProjectContext } from "../../contexts/ProjectContext";

interface CachedInsight {
  insightReviewId: string;
  noteId: string;
  synthesis: string | null;
}

export function InsightCard() {
  const navigate = useNavigate();
  const { project } = useProjectContext();
  const [insight, setInsight] = useState<CachedInsight | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    async function fetchInsight() {
      if (!project) return;
      const notebookIds = (project.settings as Record<string, unknown>)?.notebookIds as
        | string[]
        | undefined;
      if (!notebookIds?.length) {
        setLoading(false);
        return;
      }
      try {
        const notes = await ipc<Array<{ id: string; title: string; updatedAt: string }>>(
          "note_list",
          { notebookId: notebookIds[0] },
        );
        const recent = notes.slice(0, 5);
        for (const note of recent) {
          const cached = await ipc<CachedInsight | null>("note_insight_cache_get", {
            noteId: note.id,
          });
          if (cached?.synthesis) {
            setInsight(cached);
            break;
          }
        }
      } catch {
        // Silently fail — insight is optional
      } finally {
        setLoading(false);
      }
    }
    fetchInsight();
  }, [project]);

  if (loading) {
    return (
      <div className="island rounded-xl p-5">
        <div className="text-ui-xs text-fg-secondary uppercase tracking-wider mb-3">
          Latest Insight
        </div>
        <div className="h-12 bg-control-hover/20 rounded animate-pulse" />
      </div>
    );
  }

  if (!insight) {
    return (
      <div className="island rounded-xl p-5">
        <div className="text-ui-xs text-fg-secondary uppercase tracking-wider mb-3">
          Latest Insight
        </div>
        <p className="text-ui-xs text-fg-secondary">
          No insights yet. Visit the Notes tab and click "Generate Insight" on a note.
        </p>
        <button
          type="button"
          onClick={() => navigate(`/project/${project?.id}/notes`)}
          className="mt-3 text-ui-xs px-3 py-1 rounded bg-brand/10 text-brand hover:bg-brand/20 transition-colors"
        >
          Go to Notes
        </button>
      </div>
    );
  }

  return (
    <div className="island rounded-xl p-5 border border-brand/15">
      <div className="text-ui-xs text-brand uppercase tracking-wider mb-3">Latest Insight</div>
      <p className="text-ui-xs text-fg-secondary line-clamp-3 leading-relaxed">
        {insight.synthesis}
      </p>
      <div className="flex gap-2 mt-3">
        <button
          type="button"
          onClick={() => navigate(`/project/${project?.id}/notes`)}
          className="text-ui-xs px-3 py-1 rounded bg-brand/10 text-brand hover:bg-brand/20 transition-colors"
        >
          View Insight
        </button>
      </div>
    </div>
  );
}
