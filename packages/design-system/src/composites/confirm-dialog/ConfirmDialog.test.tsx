import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ConfirmDialog } from "./ConfirmDialog";

const base = {
  open: true,
  title: "Delete note?",
  message: "This cannot be undone.",
  confirmLabel: "Delete",
  cancelLabel: "Cancel",
};

describe("ConfirmDialog", () => {
  it("renders title and message in a dialog", () => {
    render(<ConfirmDialog {...base} onClose={vi.fn()} onConfirm={vi.fn()} />);

    expect(screen.getByRole("dialog", { name: "Delete note?" })).toBeInTheDocument();
    expect(screen.getByText("This cannot be undone.")).toBeInTheDocument();
  });

  it("fires onConfirm and onClose from actions", async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn();
    const onClose = vi.fn();
    render(<ConfirmDialog {...base} onClose={onClose} onConfirm={onConfirm} />);

    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onClose).toHaveBeenCalledTimes(1);

    await user.click(screen.getByRole("button", { name: "Delete" }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it("uses brand primary by default and status-danger when destructive", () => {
    const { rerender } = render(
      <ConfirmDialog {...base} onClose={vi.fn()} onConfirm={vi.fn()} />,
    );
    expect(screen.getByRole("button", { name: "Delete" }).className).toMatch(/bg-brand/);

    rerender(
      <ConfirmDialog {...base} destructive onClose={vi.fn()} onConfirm={vi.fn()} />,
    );
    expect(screen.getByRole("button", { name: "Delete" }).className).toMatch(
      /bg-status-danger/,
    );
  });
});
