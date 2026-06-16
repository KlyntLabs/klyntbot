// @vitest-environment jsdom

import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { useFocusTimer } from "../hooks/useFocusTimer";
import { FocusTimer } from "./FocusTimer";

type Timer = ReturnType<typeof useFocusTimer>;

function buildTimer(overrides: Partial<Timer> = {}): Timer {
  return {
    phase: "idle",
    paused: false,
    active: false,
    remainingSecs: null,
    totalSecs: null,
    actionTitle: null,
    showWarning: false,
    dndHint: null,
    coaching: null,
    settings: {
      focusDuration: 25,
      shortBreak: 5,
      longBreak: 15,
      longBreakAfter: 4,
      dndEnabled: false,
      soundEnabled: true,
      notificationEnabled: true,
    },
    completedSessions: 0,
    cyclePosition: 0,
    longBreakAfter: 4,
    todayStats: { sessions: 0, totalMins: 0, avgQuality: null },
    activePreset: "Custom",
    isLoading: false,
    start: vi.fn(),
    stop: vi.fn(),
    pause: vi.fn(),
    resume: vi.fn(),
    extend: vi.fn(),
    startBreak: vi.fn(),
    extendWork: vi.fn(),
    skipBreak: vi.fn(),
    takeBreak: vi.fn(),
    logDistraction: vi.fn(),
    updateSettings: vi.fn(),
    applyPreset: vi.fn(),
    dismissCoaching: vi.fn(),
    dismissDndHint: vi.fn(),
    selectTask: vi.fn(),
    selectedTaskId: null,
    selectedTaskTitle: null,
    ...overrides,
  } as unknown as Timer;
}

describe("FocusTimer", () => {
  it("renders idle duration when no session is active", () => {
    render(<FocusTimer timer={buildTimer()} onOpenSettings={vi.fn()} />);
    expect(screen.getByText("25:00")).toBeInTheDocument();
    expect(screen.getByText("Focus")).toBeInTheDocument();
  });

  it("displays remaining time during a working session", () => {
    render(
      <FocusTimer
        timer={buildTimer({
          phase: "working",
          active: true,
          remainingSecs: 120,
          totalSecs: 1500,
        })}
        onOpenSettings={vi.fn()}
      />,
    );
    expect(screen.getByText("02:00")).toBeInTheDocument();
  });

  it("renders the progress ring with the correct offset", () => {
    const remainingSecs = 120;
    const totalSecs = 1500;
    render(
      <FocusTimer
        timer={buildTimer({
          phase: "working",
          active: true,
          remainingSecs,
          totalSecs,
        })}
        onOpenSettings={vi.fn()}
      />,
    );

    const progress = (totalSecs - remainingSecs) / totalSecs;
    const RING_SIZE = 170;
    const STROKE = 5;
    const RADIUS = RING_SIZE / 2 - STROKE / 2 - 4;
    const circumference = 2 * Math.PI * RADIUS;
    const expectedOffset = circumference * (1 - progress);

    const ring = document.querySelector(".tc-ring-progress");
    expect(ring).toBeInTheDocument();
    expect(ring).toHaveAttribute("stroke-dashoffset", String(expectedOffset));
  });

  it("switches to break icon and label during a break", () => {
    render(
      <FocusTimer
        timer={buildTimer({
          phase: "break",
          active: true,
          remainingSecs: 180,
          totalSecs: 300,
        })}
        onOpenSettings={vi.fn()}
      />,
    );
    expect(screen.getByText("03:00")).toBeInTheDocument();
    expect(screen.getByText("Break")).toBeInTheDocument();
  });

  it("shows warning state", () => {
    render(
      <FocusTimer
        timer={buildTimer({
          phase: "working",
          active: true,
          remainingSecs: 30,
          totalSecs: 1500,
          showWarning: true,
        })}
        onOpenSettings={vi.fn()}
      />,
    );
    expect(screen.getByText("Focus ending soon")).toBeInTheDocument();
  });
});
