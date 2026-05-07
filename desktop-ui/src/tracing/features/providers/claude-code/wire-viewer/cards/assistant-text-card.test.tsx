import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { AssistantTextCard } from "./assistant-text-card";

describe("AssistantTextCard", () => {
  it("renders text payload", () => {
    render(<AssistantTextCard event={{ index: 0, timestamp: 0, type: "assistant.text", payload: { text: "hello world" } }} />);
    expect(screen.getByText(/hello world/)).toBeInTheDocument();
  });
});
