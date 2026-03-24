import { Progress } from "@shared/ui";
import { useNavigate } from "react-router";
import { useProjectContext } from "../../contexts/ProjectContext";

export function OkrSummaryCard() {
  const navigate = useNavigate();
  const { project, objectives } = useProjectContext();

  const top3 = objectives.slice(0, 3);

  if (top3.length === 0) {
    return (
      <button
        type="button"
        onClick={() => navigate(`/project/${project?.id ?? ""}/okr`)}
        className="glass-card rounded-xl p-5 text-left transition-colors hover:border-brand/30"
      >
        <p className="text-2xs text-muted-foreground uppercase tracking-wider mb-3">OKR Summary</p>
        <p className="text-[11px] text-muted-foreground">
          No objectives defined. Click to create one.
        </p>
      </button>
    );
  }

  return (
    <div className="glass-card rounded-xl p-5">
      <p className="text-2xs text-muted-foreground uppercase tracking-wider mb-3">OKR Summary</p>
      <div className="flex flex-col gap-3">
        {top3.map((obj) => (
          <button
            key={obj.id}
            type="button"
            onClick={() => navigate(`/project/${project?.id ?? ""}/okr`)}
            className="flex flex-col gap-1.5 text-left hover:opacity-80 transition-opacity"
          >
            <div className="flex items-center justify-between">
              <span className="text-xs text-foreground truncate max-w-[70%]">{obj.title}</span>
              <span className="text-2xs text-muted-foreground font-medium">
                {Math.round(obj.progress)}%
              </span>
            </div>
            <Progress
              value={obj.progress}
              color={obj.progress >= 70 ? "success" : obj.progress >= 40 ? "warning" : "brand"}
            />
          </button>
        ))}
      </div>
      {objectives.length > 3 && (
        <button
          type="button"
          onClick={() => navigate(`/project/${project?.id ?? ""}/okr`)}
          className="text-2xs text-brand mt-2 hover:underline"
        >
          +{objectives.length - 3} more objective{objectives.length - 3 !== 1 ? "s" : ""}
        </button>
      )}
    </div>
  );
}
