import { ActivityTimeline } from "./ActivityTimeline";
import { CoachingCard } from "./CoachingCard";
import { HealthScoreCard } from "./HealthScoreCard";
import { InsightCard } from "./InsightCard";
import { OkrSummaryCard } from "./OkrSummaryCard";
import { TaskProgressCard } from "./TaskProgressCard";
import { WorkContextCard } from "./WorkContextCard";

export function OverviewTab() {
  return (
    <div className="flex flex-col gap-4 p-6">
      {/* Row 1 — Stats */}
      <div className="grid grid-cols-3 gap-4">
        <HealthScoreCard />
        <TaskProgressCard />
        <OkrSummaryCard />
      </div>

      {/* Row 2 — Intelligence */}
      <div className="grid grid-cols-3 gap-4">
        <WorkContextCard />
        <InsightCard />
        <CoachingCard />
      </div>

      {/* Row 3 — Activity Timeline */}
      <ActivityTimeline />
    </div>
  );
}
