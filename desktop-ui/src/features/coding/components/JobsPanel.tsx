import { useMemo } from "react";
import { invoke } from "@/api/client";
import { useThreadJobs } from "@/features/coding/hooks/useThreadJobs";
import { type BashJobView, isActiveJob, useJobs } from "@/features/coding/state/jobsStore";
import { formatBytes } from "@/utils/formatting";

interface Props {
  threadId: string;
}

export function JobsPanel({ threadId }: Props) {
  useThreadJobs(threadId);
  const jobs = useJobs(threadId);

  const sorted = useMemo(
    () => [...jobs].sort((a, b) => b.started_at.localeCompare(a.started_at)),
    [jobs],
  );

  if (jobs.length === 0) {
    return (
      <div className="coding-jobs-panel coding-jobs-panel--empty">
        <h3>Background Jobs</h3>
        <p>No jobs in this thread.</p>
      </div>
    );
  }
  return (
    <div className="coding-jobs-panel">
      <h3>Background Jobs ({jobs.length})</h3>
      <ul className="coding-jobs-panel__list">
        {sorted.map((j) => (
          <JobRow key={j.id} job={j} />
        ))}
      </ul>
    </div>
  );
}

function JobRow({ job }: { job: BashJobView }) {
  const onStop = async () => {
    try {
      await invoke("coding_job_stop", {
        jobId: job.id,
        reason: "user clicked stop",
      });
    } catch (e) {
      console.warn(e);
    }
  };
  const isActive = isActiveJob(job);
  return (
    <li className={`coding-jobs-panel__row coding-jobs-panel__row--${job.status.toLowerCase()}`}>
      <div className="coding-jobs-panel__id">{job.id}</div>
      <div className="coding-jobs-panel__desc" title={job.command}>
        {job.description}
      </div>
      <div className="coding-jobs-panel__status">{job.status}</div>
      <div className="coding-jobs-panel__bytes">{formatBytes(job.total_bytes_emitted)}</div>
      {isActive && (
        <button className="coding-jobs-panel__stop" onClick={onStop} type="button">
          Stop
        </button>
      )}
    </li>
  );
}
