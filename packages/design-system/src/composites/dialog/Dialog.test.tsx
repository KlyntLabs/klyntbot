import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Dialog } from "./Dialog";

describe("Dialog", () => {
  it("renders a labelled modal when open", () => {
    render(
      <Dialog open onClose={vi.fn()} title="Edit objective">
        <p>Body</p>
      </Dialog>,
    );

    const dialog = screen.getByRole("dialog", { name: "Edit objective" });
    expect(dialog).toBeInTheDocument();
    expect(screen.getByText("Body")).toBeInTheDocument();
  });

  it("does not render content when closed", () => {
    render(
      <Dialog open={false} onClose={vi.fn()} title="Hidden">
        <p>Secret</p>
      </Dialog>,
    );

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.queryByText("Secret")).not.toBeInTheDocument();
  });

  it("calls onClose from the close button", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(
      <Dialog open onClose={onClose} title="Closeable">
        <p>Content</p>
      </Dialog>,
    );

    await user.click(screen.getByRole("button", { name: "Close dialog" }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("uses liquid-glass chrome and island body", () => {
    render(
      <Dialog open onClose={vi.fn()} title="Surfaces">
        <p>Content</p>
      </Dialog>,
    );

    const dialog = screen.getByRole("dialog");
    expect(dialog.className).toMatch(/liquid-glass/);
    expect(dialog.innerHTML).toMatch(/\bisland\b/);
    expect(dialog.innerHTML).toMatch(/border-separator/);
  });
});
