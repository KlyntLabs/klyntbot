import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import {
  applyJobsView,
  applyJobUpdate,
  type BashJobView,
  cleanupJobs,
  removeJob,
  useJobs,
} from "./jobsStore";

const fixture = (id: string, status = "Running"): BashJobView => ({
  id,
  session_id: "s1",
  agent_id: "root",
  description: id,
  command: "echo",
  cwd: "/tmp",
  status,
  started_at: "2026-05-08T00:00:00Z",
  finished_at: null,
  exit_code: null,
  failure_kind: null,
  failure_detail: null,
  failure_extracted: null,
  total_bytes_emitted: 0,
  last_polled_at: null,
  last_seen_offset: 0,
});

describe("jobsStore", () => {
  beforeEach(() => cleanupJobs("t1"));

  it("returns empty state for unknown thread", () => {
    const { result } = renderHook(() => useJobs("t1"));
    expect(result.current).toEqual([]);
  });

  it("applyJobsView sets the list", () => {
    applyJobsView("t1", [fixture("bash-aaa0000001"), fixture("bash-aaa0000002")]);
    const { result } = renderHook(() => useJobs("t1"));
    expect(result.current).toHaveLength(2);
  });

  it("applyJobUpdate replaces existing", () => {
    applyJobsView("t1", [fixture("bash-aaa0000001", "Running")]);
    applyJobUpdate("t1", fixture("bash-aaa0000001", "Completed"));
    const { result } = renderHook(() => useJobs("t1"));
    expect(result.current[0].status).toBe("Completed");
  });

  it("removeJob filters", () => {
    applyJobsView("t1", [fixture("bash-aaa0000001"), fixture("bash-aaa0000002")]);
    removeJob("t1", "bash-aaa0000001");
    const { result } = renderHook(() => useJobs("t1"));
    expect(result.current).toHaveLength(1);
    expect(result.current[0].id).toBe("bash-aaa0000002");
  });
});
