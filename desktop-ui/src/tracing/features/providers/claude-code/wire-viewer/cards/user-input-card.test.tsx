import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { UserInputCard } from "./user-input-card";

describe("UserInputCard", () => {
  it("renders text", () => {
    render(
      <UserInputCard
        event={{
          index: 0,
          timestamp: 0,
          type: "user.text",
          payload: { type: "text", text: "hi there" },
        }}
      />,
    );
    expect(screen.getByText(/hi there/)).toBeInTheDocument();
  });
  it("renders image placeholder", () => {
    render(
      <UserInputCard
        event={{ index: 0, timestamp: 0, type: "user.image", payload: { type: "image" } }}
      />,
    );
    expect(screen.getByText(/\[image\]/)).toBeInTheDocument();
  });
});
