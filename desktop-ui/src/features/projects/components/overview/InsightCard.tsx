import { Lightbulb } from "lucide-react";
import { useNavigate } from "react-router";
import { useProjectContext } from "../../contexts/ProjectContext";

export function InsightCard() {
  const navigate = useNavigate();
  const { project } = useProjectContext();

  // Iteration 1: placeholder teaser. Full implementation requires calling
  // note_insight_cache_get per project note which is complex for the overview.
  // Deferred to a follow-up when the InsightPreview component is available.

  return (
    <div className="glass-card rounded-xl p-5">
      <p className="text-[10px] text-muted-foreground uppercase tracking-wider mb-3">
        Latest Insight
      </p>

      <div className="flex flex-col items-center gap-2 py-2">
        <Lightbulb className="w-5 h-5 text-muted-foreground/50" />
        <p className="text-[11px] text-muted-foreground text-center">
          Generate insights by visiting the Notes tab
        </p>
        <button
          type="button"
          onClick={() => navigate(`/project/${project?.id ?? ""}/notes`)}
          className="text-[10px] text-brand hover:underline mt-1"
        >
          Go to Notes
        </button>
      </div>
    </div>
  );
}
