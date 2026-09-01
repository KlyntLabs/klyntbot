// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@/api/endpoints/dashboard", async () => {
  const actual = await vi.importActual<typeof import("@/api/endpoints/dashboard")>(
    "@/api/endpoints/dashboard",
  );
  return {
    ...actual,
    productivityCalendarEvents: vi.fn(),
  };
});

import { productivityCalendarEvents } from "@/api/endpoints/dashboard";
import type { CalendarEvent } from "@/bindings";
import { CalendarTrack } from "./CalendarTrack";

const mockedCalendarEvents = vi.mocked(productivityCalendarEvents);

afterEach(() => {
  cleanup();
  mockedCalendarEvents.mockReset();
});

function wrap(node: ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: 0 } } });
  return <QueryClientProvider client={client}>{node}</QueryClientProvider>;
}

function makeEvent(overrides: Partial<CalendarEvent> = {}): CalendarEvent {
  return {
    id: "evt-1",
    calendarId: "cal-1",
    title: "Standup",
    description: null,
    startedAt: "2026-04-30T09:00:00Z",
    endedAt: "2026-04-30T09:30:00Z",
    location: null,
    attendeesCount: 0,
    isRecurring: false,
    recurrenceId: null,
    source: "google",
    externalUid: "ext-1",
    sessionId: null,
    color: "#6366f1",
    syncedAt: "2026-04-30T00:00:00Z",
    createdAt: "2026-04-30T00:00:00Z",
    updatedAt: "2026-04-30T00:00:00Z",
    ...overrides,
  };
}

describe("CalendarTrack", () => {
  it("renders empty state when no events", async () => {
    mockedCalendarEvents.mockResolvedValue([]);
    const { container } = render(
      wrap(
        <CalendarTrack
          date="2026-04-30"
          hourHeight={60}
          selectedEventId={null}
          onSelectEvent={() => {}}
        />,
      ),
    );
    await waitFor(() => {
      expect(container.textContent).toContain("No calendar events for this day");
    });
  });

  it("renders event blocks with titles", async () => {
    mockedCalendarEvents.mockResolvedValue([makeEvent({ title: "Standup" })]);
    render(
      wrap(
        <CalendarTrack
          date="2026-04-30"
          hourHeight={60}
          selectedEventId={null}
          onSelectEvent={() => {}}
        />,
      ),
    );
    await waitFor(() => {
      expect(screen.getByText("Standup")).toBeTruthy();
    });
  });

  it("calls onSelectEvent with the event on click", async () => {
    const event = makeEvent({ id: "evt-1", title: "Standup" });
    mockedCalendarEvents.mockResolvedValue([event]);
    const onSelectEvent = vi.fn();
    render(
      wrap(
        <CalendarTrack
          date="2026-04-30"
          hourHeight={60}
          selectedEventId={null}
          onSelectEvent={onSelectEvent}
        />,
      ),
    );
    await waitFor(() => {
      expect(screen.getByText("Standup")).toBeTruthy();
    });
    const standupButton = screen.getByText("Standup").closest("button");
    if (!standupButton) throw new Error("Standup button not found");
    fireEvent.click(standupButton);
    expect(onSelectEvent).toHaveBeenCalledWith(expect.objectContaining({ id: "evt-1" }));
  });

  it("calls onSelectEvent with null when clicking a selected event", async () => {
    const event = makeEvent({ id: "evt-1", title: "Standup" });
    mockedCalendarEvents.mockResolvedValue([event]);
    const onSelectEvent = vi.fn();
    render(
      wrap(
        <CalendarTrack
          date="2026-04-30"
          hourHeight={60}
          selectedEventId="evt-1"
          onSelectEvent={onSelectEvent}
        />,
      ),
    );
    await waitFor(() => {
      expect(screen.getByText("Standup")).toBeTruthy();
    });
    const standupButton2 = screen.getByText("Standup").closest("button");
    if (!standupButton2) throw new Error("Standup button not found");
    fireEvent.click(standupButton2);
    expect(onSelectEvent).toHaveBeenCalledWith(null);
  });

  it("applies selected modifier class when selectedEventId matches", async () => {
    const event = makeEvent({ id: "evt-1", title: "Standup" });
    mockedCalendarEvents.mockResolvedValue([event]);
    render(
      wrap(
        <CalendarTrack
          date="2026-04-30"
          hourHeight={60}
          selectedEventId="evt-1"
          onSelectEvent={() => {}}
        />,
      ),
    );
    await waitFor(() => {
      const btn = screen.getByText("Standup").closest("button");
      expect(btn?.className).toContain("dashboard__calendar-event--selected");
    });
  });

  it("renders two overlapping events side-by-side via computeOverlapLayout", async () => {
    mockedCalendarEvents.mockResolvedValue([
      makeEvent({
        id: "e1",
        title: "Meeting A",
        startedAt: "2026-05-02T10:00:00",
        endedAt: "2026-05-02T11:00:00",
        color: "#4285F4",
      }),
      makeEvent({
        id: "e2",
        title: "Meeting B",
        startedAt: "2026-05-02T10:30:00",
        endedAt: "2026-05-02T11:30:00",
        color: "#FF0000",
      }),
    ]);
    render(
      wrap(
        <CalendarTrack
          date="2026-05-02"
          hourHeight={60}
          selectedEventId={null}
          onSelectEvent={() => {}}
        />,
      ),
    );
    await waitFor(() => expect(screen.getByText("Meeting A")).toBeTruthy());
    expect(screen.getByText("Meeting B")).toBeTruthy();
    // Both blocks present — overlap layout should give them differing left/width inline styles
    const blocks = screen.getAllByRole("button");
    const styleA = blocks[0].getAttribute("style") ?? "";
    const styleB = blocks[1].getAttribute("style") ?? "";
    // Distinct left percentages indicate overlap layout was applied
    expect(styleA).not.toEqual(styleB);
  });

  it("shows empty state when productivityCalendarEvents returns empty", async () => {
    mockedCalendarEvents.mockResolvedValue([]);
    const { container } = render(
      wrap(
        <CalendarTrack
          date="2026-05-02"
          hourHeight={60}
          selectedEventId={null}
          onSelectEvent={() => {}}
        />,
      ),
    );
    await waitFor(() => expect(container.textContent).toContain("No calendar events for this day"));
  });
});
