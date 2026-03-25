import { useMutation } from "@shared/hooks/useMutation";
import { invalidateQueries } from "@shared/hooks/useQuery";
import { Check, Sparkles, X } from "lucide-react";

export interface MetaRule {
  id: string;
  triggerCondition: string;
  action: Record<string, unknown>;
  source: string;
  effectivenessScore: number;
  status: string;
  signalCount: number;
}

interface MetaRulesSectionProps {
  activeRules: MetaRule[];
  pendingRules: MetaRule[];
}

export function MetaRulesSection({ activeRules, pendingRules }: MetaRulesSectionProps) {
  const { mutate: approve } = useMutation<void, { ruleId: string }>("approve_meta_rule");
  const { mutate: dismiss } = useMutation<void, { ruleId: string }>("dismiss_meta_rule");

  if (activeRules.length === 0 && pendingRules.length === 0) return null;

  return (
    <div className="flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-muted-foreground flex items-center gap-1.5">
        <Sparkles className="size-3.5" />
        Rules About How I Think
      </h2>

      {pendingRules.map((rule) => (
        <div key={rule.id} className="glass-card rounded-xl p-4 border border-accent/20">
          <p className="text-[12px] text-foreground mb-1">
            I think I should: &ldquo;{rule.triggerCondition}&rdquo;
          </p>
          <p className="text-[11px] text-muted-foreground mb-3">Sound good?</p>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={async () => {
                await approve({ ruleId: rule.id });
                invalidateQueries("get_mirror");
              }}
              className="flex items-center gap-1 px-2.5 py-1 rounded-md text-2xs text-success bg-success/10 hover:bg-success/20 transition-colors"
            >
              <Check className="size-3" />
              Approve
            </button>
            <button
              type="button"
              onClick={async () => {
                await dismiss({ ruleId: rule.id });
                invalidateQueries("get_mirror");
              }}
              className="flex items-center gap-1 px-2.5 py-1 rounded-md text-2xs text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors"
            >
              <X className="size-3" />
              Dismiss
            </button>
          </div>
        </div>
      ))}

      {activeRules.map((rule) => (
        <div key={rule.id} className="glass-card rounded-xl p-4 opacity-80">
          <div className="flex items-center justify-between">
            <p className="text-[11px] text-foreground">{rule.triggerCondition}</p>
            <span className="text-2xs text-success px-1.5 py-0.5 rounded bg-success/10">
              active
            </span>
          </div>
          {rule.signalCount > 0 && (
            <p className="text-2xs text-dim mt-1">Triggered {rule.signalCount} times</p>
          )}
        </div>
      ))}
    </div>
  );
}
