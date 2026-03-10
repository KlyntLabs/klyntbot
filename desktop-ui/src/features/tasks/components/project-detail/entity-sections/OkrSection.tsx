import { CollapsibleSection } from "@shared/components";
import { useQuery } from "@shared/hooks/useQuery";
import type { Objective } from "@shared/types";
import { Progress } from "@shared/ui";
import { Target } from "lucide-react";
import { useNavigate } from "react-router";

interface OkrSectionProps {
  projectId: string;
  defaultOpen?: boolean;
}

export function OkrSection({ projectId, defaultOpen }: OkrSectionProps) {
  const navigate = useNavigate();
  const { data: objectives } = useQuery<Objective[]>(
    "objective_list",
    { project_id: projectId },
    [],
  );

  return (
    <CollapsibleSection
      title="OKRs"
      icon={<Target className="w-3.5 h-3.5 text-brand" strokeWidth={1.5} />}
      count={objectives.length || null}
      defaultOpen={defaultOpen}
    >
      {objectives.length === 0 ? (
        <p className="text-[11px] text-dim font-light py-2">No objectives</p>
      ) : (
        <div className="space-y-2">
          {objectives.map((obj) => (
            <button
              key={obj.id}
              type="button"
              onClick={() => navigate(`/objective/${obj.id}`)}
              className="w-full text-left flex items-center gap-2 hover:bg-white/[0.04] rounded-md px-2 py-1.5 transition-colors"
            >
              <div className="flex-1 min-w-0">
                <p className="text-[12px] font-light text-secondary truncate">{obj.title}</p>
                <div className="flex items-center gap-2 mt-1">
                  <div className="flex-1">
                    <Progress value={obj.progress} />
                  </div>
                  <span className="text-[10px] text-muted tabular-nums">{obj.progress}%</span>
                </div>
              </div>
            </button>
          ))}
        </div>
      )}
    </CollapsibleSection>
  );
}
