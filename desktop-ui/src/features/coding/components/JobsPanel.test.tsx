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
  tty: false,
  tty_rows: null,
  tty_cols: null,
  attached_user_at: null,
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

  it("renders Attach button on tty=true running jobs", () => {
    const pty = fixture("bash-pty0000001", "Running");
    pty.tty = true;
    applyJobsView("t1", [pty]);
    render(<JobsPanel threadId="t1" />);
    expect(screen.getByRole("button", { name: "Attach" })).toBeInTheDocument();
  });

  it("does NOT render Attach button on tty=false jobs", () => {
    applyJobsView("t1", [fixture("bash-aaa0000001", "Running")]);
    render(<JobsPanel threadId="t1" />);
    expect(screen.queryByRole("button", { name: "Attach" })).toBeNull();
  });
});
