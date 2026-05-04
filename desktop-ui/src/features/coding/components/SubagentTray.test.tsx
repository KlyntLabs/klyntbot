import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { SubagentTray } from "./SubagentTray";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => []) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

describe("SubagentTray", () => {
  it("renders nothing when empty", () => {
    const { container } = render(<SubagentTray threadId="t1" />);
    expect(container.firstChild).toBeNull();
  });
});
