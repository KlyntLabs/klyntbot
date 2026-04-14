import { CoachingOverviewPage, HistoryPage, PatternsPage } from "@features/coaching";
import { useState } from "react";

type CoachingSection = "overview" | "patterns" | "history";

const sections: { id: CoachingSection; label: string }[] = [
  { id: "overview", label: "Overview" },
  { id: "patterns", label: "Patterns" },
  { id: "history", label: "History" },
];

export function CoachingDetail() {
  const [active, setActive] = useState<CoachingSection>("overview");

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-1.5">
        {sections.map((s) => (
          <button
            key={s.id}
            type="button"
            onClick={() => setActive(s.id)}
            className={`px-3 py-1.5 rounded-lg text-xs font-light transition-colors ${
              active === s.id
                ? "bg-surface-low text-foreground"
                : "text-muted-foreground hover:text-foreground"
            }`}
          >
            {s.label}
          </button>
        ))}
      </div>
      {active === "overview" && <CoachingOverviewPage />}
      {active === "patterns" && <PatternsPage />}
      {active === "history" && <HistoryPage />}
    </div>
  );
}
