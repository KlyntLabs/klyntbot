import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { FocusBanner } from "./FocusBanner";

describe("FocusBanner", () => {
  it("renders nothing when activeTask is null", () => {
    const { container } = render(<FocusBanner activeTask={null} onEndFocus={() => {}} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders task title and timer when active", () => {
    const focusedAt = new Date(Date.now() - 90_000).toISOString();
    render(
      <FocusBanner
        activeTask={{ id: "task-1", title: "Write tests", focusedAt }}
        onEndFocus={() => {}}
      />,
    );
    expect(screen.getByText("Write tests")).toBeInTheDocument();
    expect(screen.getByText(/\d{2}:\d{2}/)).toBeInTheDocument();
  });

  it("calls onEndFocus with task id when End Focus clicked", () => {
    const onEnd = vi.fn();
    const focusedAt = new Date().toISOString();
    render(
      <FocusBanner
        activeTask={{ id: "task-1", title: "Write tests", focusedAt }}
        onEndFocus={onEnd}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /end focus/i }));
    expect(onEnd).toHaveBeenCalledWith("task-1");
  });
});
