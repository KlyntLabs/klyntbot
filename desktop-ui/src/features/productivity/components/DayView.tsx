import { useEvent } from "@shared/hooks/useEvent";
import { useQuery } from "@shared/hooks/useQuery";
import { todayISO } from "@shared/lib/dates";
import type { ProductivitySummary } from "@shared/types";
import { useMemo } from "react";
import { buildBreakdownSegments } from "../lib/constants";
import { ActivityFeed } from "./ActivityFeed";
import { AiSummaryCard } from "./AiSummaryCard";
import { AutoFocusToast } from "./AutoFocusToast";
import { BreakdownDonuts } from "./BreakdownDonuts";
import { CategoriesList } from "./CategoriesList";
import { DistractionBanner } from "./DistractionBanner";
import { FocusSessionsList } from "./FocusSessionsList";
import { FocusStateIndicator } from "./FocusStateIndicator";
import { GoalsProgress } from "./GoalsProgress";
import { InsightCardList } from "./InsightCardList";
import { LearnedRulesCard } from "./LearnedRulesCard";
import { LiveScoreRing } from "./LiveScoreRing";
import { ProjectsCard } from "./ProjectsCard";
import { TimeEntrySection } from "./TimeEntrySection";
import { TimelineBar } from "./Timeline";
import { TopApps } from "./TopApps";
import { WorkHoursCard } from "./WorkHoursCard";

interface DayViewProps {
  date: string;
}

export function DayView({ date }: DayViewProps) {
  const isToday = date === todayISO();

  // Today: use live endpoint; past dates: query range for that single day
  const { data: todaySummary, refetch: refetchToday } = useQuery<ProductivitySummary | null>(
    "productivity_today",
    isToday ? undefined : null,
    null,
  );
  const rangeArgs = useMemo(
    () => (isToday ? null : { start_date: date, end_date: date }),
    [isToday, date],
  );
  const { data: rangeSummaries, refetch: refetchRange } = useQuery<ProductivitySummary[]>(
    "productivity_summary_range",
    rangeArgs,
    [],
  );

  const summary = isToday ? todaySummary : (rangeSummaries[0] ?? null);
  const refetch = isToday ? refetchToday : refetchRange;

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
      {/* Row 1: Timeline + focus state (full width) */}
      <div className="col-span-3 flex items-center gap-3">
        <div className="flex-1">
          <TimelineBar date={date} />
        </div>
        <FocusStateIndicator />
      </div>

      {/* Auto-focus detection toast (full width, auto-hides) */}
      <AutoFocusToast />

      {/* Distraction warning — appears when distracting time detected */}
      <DistractionBanner summary={summary} />

      {/* Row 2-3: Left column */}
      <div className="flex flex-col gap-4">
        <ActivityFeed />
        <FocusSessionsList date={date} />
      </div>

      {/* Row 2-3: Center column */}
      <div className="flex flex-col gap-4">
        <TopApps apps={summary?.topApps ?? []} />
        <ProjectsCard
          projects={summary?.topProjects ?? []}
          totalSecs={summary?.totalActiveSecs ?? 0}
        />
        <TimeEntrySection date={date} />
      </div>

      {/* Row 2-3: Right column */}
      <div className="flex flex-col gap-4">
        <WorkHoursCard totalActiveSecs={summary?.totalActiveSecs ?? 0} />
        <div className="glass-card p-4 flex items-center justify-center relative">
          <LiveScoreRing summary={summary} />
        </div>
        <BreakdownDonuts segments={breakdownSegments} totalSecs={summary?.totalActiveSecs ?? 0} />
        <CategoriesList
          categories={summary?.topCategories ?? []}
          totalSecs={summary?.totalActiveSecs ?? 0}
        />
        <AiSummaryCard summary={summary?.aiSummary ?? null} />
        <InsightCardList date={date} />
        <LearnedRulesCard />
      </div>

      {/* Row 4: Goals (spans left+center) */}
      <div className="col-span-2">
        <GoalsProgress />
      </div>
    </div>
  );
}
