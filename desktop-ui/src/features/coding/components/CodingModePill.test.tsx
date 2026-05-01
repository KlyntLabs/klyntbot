// @vitest-environment jsdom

import { render, screen } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { CodingModePill } from "./CodingModePill";

vi.mock("@/api/client", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "chat_get_session") return { conversationType: "general" };
    return null;
  }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));

describe("CodingModePill", () => {
  test("renders General when mode is general", async () => {
    render(<CodingModePill threadId="t1" />);
    const btn = await screen.findByRole("button");
    expect(btn.textContent).toBe("General");
  });

  test("disabled when threadId is null", () => {
    render(<CodingModePill threadId={null} />);
    const btn = screen.getByRole("button");
    expect(btn).toBeDisabled();
  });
});
