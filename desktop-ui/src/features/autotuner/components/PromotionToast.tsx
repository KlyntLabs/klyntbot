import { useNavigate } from "react-router";

interface PromotionToastProps {
  impact: string;
  onDismiss: () => void;
}

export function PromotionToast({ impact, onDismiss }: PromotionToastProps) {
  const navigate = useNavigate();

  return (
    <div className="animate-[slideIn_0.2s_ease-out]">
      <div
        className="glass-card p-4 flex items-center gap-3 border-l-2"
        style={{ borderLeftColor: "var(--success)" }}
      >
        <div className="flex-1 min-w-0">
          <span className="text-[12px] font-medium text-foreground">
            I just improved how I understand you
          </span>
          <p className="text-[11px] font-light text-muted-foreground mt-0.5">{impact}</p>
        </div>

        <div className="flex items-center gap-2 flex-shrink-0">
          <button
            type="button"
            onClick={() => {
              onDismiss();
              navigate("/settings/general");
            }}
            className="text-[11px] font-medium px-3 py-1.5 rounded-lg bg-success/15 text-success hover:bg-success/25 transition-colors"
          >
            Show me
          </button>
          <button
            type="button"
            onClick={onDismiss}
            className="text-[11px] font-medium px-3 py-1.5 rounded-lg bg-[var(--surface-glass-subtle)] text-muted-foreground hover:bg-[var(--surface-glass-subtle-hover)] transition-colors"
          >
            Dismiss
          </button>
        </div>
      </div>
    </div>
  );
}
