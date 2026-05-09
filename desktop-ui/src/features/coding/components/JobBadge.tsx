import { useMemo } from "react";
import { isActiveJob, useJobs } from "@/features/coding/state/jobsStore";

export function JobBadge({ threadId }: { threadId: string }) {
  const jobs = useJobs(threadId);
  const active = useMemo(() => jobs.reduce((c, j) => c + (isActiveJob(j) ? 1 : 0), 0), [jobs]);
  if (active === 0) return null;
  return (
    <span className="coding-jobs-badge" title={`${active} active background job(s)`}>
      <span className="coding-jobs-badge__spinner" />
      {active}
    </span>
  );
}
