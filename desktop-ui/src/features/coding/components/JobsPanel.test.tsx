import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { BashJobView } from "@/features/coding/state/jobsStore";
import { applyJobsView, cleanupJobs } from "@/features/coding/state/jobsStore";
import { JobsPanel } from "./JobsPanel";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("@/api/client", () => ({
  invoke: vi.fn().mockResolvedValue({ jobs: [] }),
}));

const fixture = (id: string, status = "Running"): BashJobView => ({
  id,
  session_id: "t1",
  agent_id: "root",
  description: `desc ${id}`,
  command: "echo",
  cwd: "/tmp",
  status,
  started_at: "2026-05-08T00:00:00Z",
  finished_at: null,
  exit_code: null,
  failure_kind: null,
  failure_detail: null,
  failure_extracted: null,
  total_bytes_emitted: 1024,
  last_polled_at: null,
  last_seen_offset: 0,
});

describe("JobsPanel", () => {
  beforeEach(() => cleanupJobs("t1"));

  it("renders empty state", () => {
    render(<JobsPanel threadId="t1" />);
    expect(screen.getByText(/No jobs in this thread/i)).toBeInTheDocument();
  });

  it("renders 2 jobs", () => {
    applyJobsView("t1", [fixture("bash-aaa0000001"), fixture("bash-aaa0000002")]);
    render(<JobsPanel threadId="t1" />);
    expect(screen.getByText("Background Jobs (2)")).toBeInTheDocument();
    expect(screen.getByText("desc bash-aaa0000001")).toBeInTheDocument();
  });

  it("shows Stop button only for active jobs", () => {
    applyJobsView("t1", [
      fixture("bash-aaa0000001", "Running"),
      fixture("bash-aaa0000002", "Completed"),
    ]);
    render(<JobsPanel threadId="t1" />);
    const stopButtons = screen.getAllByRole("button", { name: /stop/i });
    expect(stopButtons).toHaveLength(1);
  });
});
