import { useEvent } from "../../hooks/useEvent";
import { useQuery } from "../../hooks/useQuery";
import type { ProductivitySummary } from "../../lib/types";
import { ActivityFeed } from "./ActivityFeed";
import { AiSummaryCard } from "./AiSummaryCard";
import { BreakdownDonuts } from "./BreakdownDonuts";
import { CategoriesList } from "./CategoriesList";
import { DistractionBanner } from "./DistractionBanner";
import { FocusSessionsList } from "./FocusSessionsList";
import { GoalsProgress } from "./GoalsProgress";
import { LearnedRulesCard } from "./LearnedRulesCard";
import { PomodoroTimer } from "./PomodoroTimer";
import { ProductivityScoreRing } from "./ProductivityScoreRing";
import { buildBreakdownSegments } from "./shared";
import { TimeEntrySection } from "./TimeEntrySection";
import { TimelineBar } from "./Timeline";
import { TopApps } from "./TopApps";
import { WorkHoursCard } from "./WorkHoursCard";

interface DayViewProps {
  date: string;
}

export function DayView({ date }: DayViewProps) {
  const { data: summary, refetch } = useQuery<ProductivitySummary | null>(
    "productivity_today",
    undefined,
    null,
  );

  useEvent<{ entityKind: string }>("entity:updated", (payload) => {
    const k = payload?.entityKind;
    if (k === "productivity" || k === "focus_session") refetch();
  });

  const breakdownSegments = summary
    ? buildBreakdownSegments(
        summary.totalActiveSecs,
        summary.totalFocusSecs,
        summary.totalBreakSecs,
      )
    : [];

  return (
    <div className="grid grid-cols-3 gap-4 auto-rows-min">
      {/* Row 1: Timeline (full width) */}
      <TimelineBar date={date} />

      {/* Distraction warning — appears when distracting time detected */}
      <DistractionBanner summary={summary} />

      {/* Row 2-3: Left column */}
      <div className="flex flex-col gap-4">
        <PomodoroTimer />
        <ActivityFeed />
      </div>

      {/* Row 2-3: Center column */}
      <div className="flex flex-col gap-4">
        <FocusSessionsList date={date} />
        <TopApps apps={summary?.topApps ?? []} />
        <TimeEntrySection date={date} />
      </div>

      {/* Row 2-3: Right column */}
      <div className="flex flex-col gap-4">
        <WorkHoursCard totalActiveSecs={summary?.totalActiveSecs ?? 0} />
        <div className="glass-card p-4 flex items-center justify-center relative">
          <ProductivityScoreRing score={summary?.productivityScore ?? 0} summary={summary} />
        </div>
        <BreakdownDonuts segments={breakdownSegments} totalSecs={summary?.totalActiveSecs ?? 0} />
        <CategoriesList
          categories={summary?.topCategories ?? []}
          totalSecs={summary?.totalActiveSecs ?? 0}
        />
        <AiSummaryCard summary={summary?.aiSummary ?? null} />
        <LearnedRulesCard />
      </div>

      {/* Row 4: Goals (spans left+center) */}
      <div className="col-span-2">
        <GoalsProgress />
      </div>
    </div>
  );
}
