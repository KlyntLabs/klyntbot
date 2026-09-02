import { useMutation } from "@shared/hooks/useMutation";
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
  onRuleAction?: () => void;
}

export function MetaRulesSection({
  activeRules,
  pendingRules,
  onRuleAction,
}: MetaRulesSectionProps) {
  const { mutate: approve } = useMutation<void, { ruleId: string }>("approve_meta_rule");
  const { mutate: dismiss } = useMutation<void, { ruleId: string }>("dismiss_meta_rule");

  if (activeRules.length === 0 && pendingRules.length === 0) return null;

  return (
    <div className="flex flex-col gap-3">
      <h2 className="text-ui font-medium text-fg-secondary flex items-center gap-1.5">
        <Sparkles className="size-3.5" />
        Rules About How I Think
      </h2>

      {pendingRules.map((rule) => (
        <div key={rule.id} className="island rounded-xl p-4 border border-brand/20">
          <p className="text-ui-sm text-fg mb-1">
            I think I should: &ldquo;{rule.triggerCondition}&rdquo;
          </p>
          <p className="text-ui-xs text-fg-secondary mb-3">Sound good?</p>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={async () => {
                await approve({ ruleId: rule.id });
                onRuleAction?.();
              }}
              className="flex items-center gap-1 px-2.5 py-1 rounded-md text-ui-xs text-status-success bg-status-success/10 hover:bg-status-success/20 transition-colors"
            >
              <Check className="size-3" />
              Approve
            </button>
            <button
              type="button"
              onClick={async () => {
                await dismiss({ ruleId: rule.id });
                onRuleAction?.();
              }}
              className="flex items-center gap-1 px-2.5 py-1 rounded-md text-ui-xs text-fg-secondary hover:text-status-danger hover:bg-status-danger/10 transition-colors"
            >
              <X className="size-3" />
              Dismiss
            </button>
          </div>
        </div>
      ))}

      {activeRules.map((rule) => (
        <div key={rule.id} className="island rounded-xl p-4 opacity-80">
          <div className="flex items-center justify-between">
            <p className="text-ui-xs text-fg">{rule.triggerCondition}</p>
            <span className="text-ui-xs text-status-success px-1.5 py-0.5 rounded bg-status-success/10">
              active
            </span>
          </div>
          {rule.signalCount > 0 && (
            <p className="text-ui-xs text-fg-dim mt-1">Triggered {rule.signalCount} times</p>
          )}
        </div>
      ))}
    </div>
  );
}
