import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { BashJobView } from "@/features/coding/state/jobsStore";
import { applyJobsView, cleanupJobs } from "@/features/coding/state/jobsStore";
import { JobBadge } from "./JobBadge";

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

describe("JobBadge", () => {
  beforeEach(() => cleanupJobs("t1"));

  it("returns null when no jobs", () => {
    const { container } = render(<JobBadge threadId="t1" />);
    expect(container.firstChild).toBeNull();
  });

  it("returns null when no active jobs", () => {
    applyJobsView("t1", [
      fixture("bash-aaa0000001", "Completed"),
      fixture("bash-aaa0000002", "Failed"),
    ]);
    const { container } = render(<JobBadge threadId="t1" />);
    expect(container.firstChild).toBeNull();
  });

  it("shows active count with spinner", () => {
    applyJobsView("t1", [
      fixture("bash-aaa0000001", "Running"),
      fixture("bash-aaa0000002", "Starting"),
      fixture("bash-aaa0000003", "Completed"),
    ]);
    render(<JobBadge threadId="t1" />);
    expect(screen.getByText("2")).toBeInTheDocument();
    expect(document.querySelector(".coding-jobs-badge__spinner")).toBeInTheDocument();
  });

  it("shows single active job count", () => {
    applyJobsView("t1", [fixture("bash-aaa0000001", "Running")]);
    render(<JobBadge threadId="t1" />);
    expect(screen.getByText("1")).toBeInTheDocument();
  });
});
