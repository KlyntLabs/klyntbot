import { ProgressRing } from "@shared/ui";
import { useNavigate } from "react-router";
import { useProjectContext } from "../../contexts/ProjectContext";
import { useHealthScore } from "../../hooks/useHealthScore";

const STATUS_TEXT: Record<string, string> = {
  green: "On Track",
  yellow: "Needs Attention",
  red: "At Risk",
};

export function HealthScoreCard() {
  const navigate = useNavigate();
  const { project, objectives, tasks } = useProjectContext();
  const health = useHealthScore(objectives, tasks, project?.id);

  const allKrs = objectives.flatMap((o) => o.keyResults ?? []);

  return (
    <button
      type="button"
      onClick={() => navigate(`/project/${project?.id ?? ""}/okr`)}
      className="island rounded-xl p-5 text-left transition-colors hover:border-brand/30"
    >
      <p className="text-ui-xs text-fg-secondary uppercase tracking-wider mb-3">Health Score</p>
      <div className="flex items-center gap-4">
        <ProgressRing progress={health.score} size="lg" gradient />
        <div className="flex flex-col gap-1">
          <span className="text-2xl font-bold text-fg">{health.score}%</span>
          <span
            className="text-ui-xs font-medium"
            style={{
              color:
                health.color === "green"
                  ? "var(--ds-status-success)"
                  : health.color === "yellow"
                    ? "var(--ds-status-warning)"
                    : "var(--ds-status-danger)",
            }}
          >
            {STATUS_TEXT[health.color] ?? "Unknown"}
          </span>
          <span className="text-ui-xs text-fg-secondary">
            {allKrs.length} key result{allKrs.length !== 1 ? "s" : ""} tracked
          </span>
        </div>
      </div>
    </button>
  );
}
