import { useMemo, useState } from "react";
import { invoke } from "@/api/client";
import { AttachTerminal } from "@/features/coding/components/AttachTerminal";
import { useThreadJobs } from "@/features/coding/hooks/useThreadJobs";
import { type BashJobView, isActiveJob, useJobs } from "@/features/coding/state/jobsStore";
import { formatBytes } from "@/utils/formatting";

interface Props {
  threadId: string;
}

export function JobsPanel({ threadId }: Props) {
  useThreadJobs(threadId);
  const jobs = useJobs(threadId);
  const [attachedJobId, setAttachedJobId] = useState<string | null>(null);

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
          <JobRow
            key={j.id}
            job={j}
            attached={attachedJobId === j.id}
            onAttach={() => setAttachedJobId(j.id)}
            onDetach={() => setAttachedJobId(null)}
          />
        ))}
      </ul>
      {attachedJobId && (
        <div className="coding-jobs-panel__attach-pane">
          <AttachTerminal threadId={threadId} jobId={attachedJobId} />
        </div>
      )}
    </div>
  );
}

interface JobRowProps {
  job: BashJobView;
  attached: boolean;
  onAttach: () => void;
  onDetach: () => void;
}

function JobRow({ job, attached, onAttach, onDetach }: JobRowProps) {
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
  const supportsAttach = job.tty && isActive;
  return (
    <li className={`coding-jobs-panel__row coding-jobs-panel__row--${job.status.toLowerCase()}`}>
      <div className="coding-jobs-panel__id">{job.id}</div>
      <div className="coding-jobs-panel__desc" title={job.command}>
        {job.description}
      </div>
      <div className="coding-jobs-panel__status">{job.status}</div>
      <div className="coding-jobs-panel__bytes">{formatBytes(job.total_bytes_emitted)}</div>
      {supportsAttach && !attached && (
        <button className="coding-jobs-panel__attach" onClick={onAttach} type="button">
          Attach
        </button>
      )}
      {supportsAttach && attached && (
        <button className="coding-jobs-panel__detach" onClick={onDetach} type="button">
          Detach
        </button>
      )}
      {isActive && (
        <button className="coding-jobs-panel__stop" onClick={onStop} type="button">
          Stop
        </button>
      )}
    </li>
  );
}
