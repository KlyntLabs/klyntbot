// @vitest-environment jsdom

import { render, screen } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { SandboxStatusPill } from "./SandboxStatusPill";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));

describe("SandboxStatusPill", () => {
  test("renders idle by default", () => {
    render(<SandboxStatusPill threadId="t1" />);
    expect(screen.getByText("⌛ idle")).toBeTruthy();
  });

  test("renders disabled state when threadId is null", () => {
    render(<SandboxStatusPill threadId={null} />);
    expect(screen.getByText("⌛ idle")).toBeTruthy();
  });
});
