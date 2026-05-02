import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { SessionCard } from "./SessionCard";
import type { SessionSummary } from "./types";

const sample: SessionSummary = {
  sessionId: "sess-fixture-001",
  providerId: "default",
  sourceDir: "/x",
  cwd: "/Users/me/proj",
  projectBasename: "proj",
  customTitle: "tạo cho tôi SUI counter",
  startedAt: "2026-04-29T00:00:00Z",
  lastEventAt: "2026-04-29T01:00:00Z",
  sizeBytes: 15_360,
  turnCount: 1,
  stepCount: 1,
  toolCallCount: 0,
  errorCount: 1,
  subagentCount: 0,
  hasWire: true,
  hasContext: true,
  imported: false,
  workDirHash: "",
  hasState: false,
  wireSize: 0,
  contextSize: 0,
  stateSize: 0,
  totalSize: 15360,
  metadata: null,
};

describe("SessionCard", () => {
  it("renders id, title, counts, and error pill", () => {
    render(<SessionCard summary={sample} onClick={() => {}} />);
    expect(screen.getByText(/sess-fix/)).toBeInTheDocument();
    expect(screen.getByText(/tạo cho tôi SUI counter/)).toBeInTheDocument();
    expect(screen.getByText(/1 turn/)).toBeInTheDocument();
    expect(screen.getByText(/1 error/)).toBeInTheDocument();
  });

  it("renders 'Untitled Session' fallback when no title", () => {
    render(<SessionCard summary={{ ...sample, customTitle: null as any }} onClick={() => {}} />);
    expect(screen.getByText(/Untitled Session/)).toBeInTheDocument();
  });

  it("shows wire and context badges", () => {
    render(<SessionCard summary={sample} onClick={() => {}} />);
    expect(screen.getByText("wire")).toBeInTheDocument();
    expect(screen.getByText("context")).toBeInTheDocument();
  });

  it("hides error count when zero", () => {
    render(<SessionCard summary={{ ...sample, errorCount: 0 }} onClick={() => {}} />);
    expect(screen.queryByText(/error/)).not.toBeInTheDocument();
  });
});
