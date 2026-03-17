import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import type { LearnedRule } from "@shared/types";
import { Shield, Trash2 } from "lucide-react";

export function LearnedRulesCard() {
  const { data: rules, refetch } = useQuery<LearnedRule[]>(
    "distraction_learned_rules",
    undefined,
    [],
  );
  const { mutate: deleteRule } = useMutation("distraction_delete_rule");

  if (rules.length === 0) return null;

  const handleDelete = async (id: number) => {
    await deleteRule({ id } as Record<string, unknown>);
    refetch();
  };

  return (
    <div className="glass-card p-4 flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-muted-foreground flex items-center gap-2">
        <Shield className="w-3.5 h-3.5 text-muted-foreground" />
        Learned Rules
        <span className="text-[10px] text-dim font-light ml-auto">{rules.length} rules</span>
      </h2>

      <div className="flex flex-col gap-1.5 max-h-48 overflow-y-auto">
        {rules.map((r) => (
          <div
            key={r.id}
            className="group flex items-center justify-between py-1.5 text-[12px] font-light"
          >
            <div className="flex items-center gap-2 min-w-0">
              <span
                className={`text-[10px] px-1.5 py-0.5 rounded ${
                  r.classification === "educational" || r.classification === "work_research"
                    ? "bg-success/10 text-success"
                    : "bg-destructive/10 text-destructive"
                }`}
              >
                {r.classification.replace("_", " ")}
              </span>
              <span className="text-foreground truncate">{r.pattern}</span>
              <span className="text-dim flex-shrink-0">&times;{r.hitCount}</span>
            </div>
            <button
              type="button"
              onClick={() => handleDelete(r.id)}
              className="w-5 h-5 rounded flex items-center justify-center text-transparent group-hover:text-muted-foreground hover:!text-destructive transition-colors flex-shrink-0"
            >
              <Trash2 className="w-3 h-3" />
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}
