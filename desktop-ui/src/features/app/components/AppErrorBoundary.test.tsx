// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AppErrorBoundary } from "./AppErrorBoundary";

function Boom({ message = "selector exploded" }: { message?: string }): never {
  throw new Error(message);
}

beforeEach(() => {
  vi.spyOn(console, "error").mockImplementation(() => {});
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("AppErrorBoundary", () => {
  it("renders children when no error is thrown", () => {
    render(
      <AppErrorBoundary surface="main">
        <div>healthy child</div>
      </AppErrorBoundary>,
    );
    expect(screen.getByText("healthy child")).toBeTruthy();
  });

  it("renders the fallback with the error message when a child throws", () => {
    render(
      <AppErrorBoundary surface="main">
        <Boom message="boom: maximum update depth exceeded" />
      </AppErrorBoundary>,
    );
    expect(screen.getByRole("alert")).toBeTruthy();
    expect(screen.getByText(/Something went wrong/i)).toBeTruthy();
    expect(screen.getByText(/boom: maximum update depth exceeded/)).toBeTruthy();
    expect(screen.getByRole("button", { name: /Reload app/i })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Try again/i })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Copy details/i })).toBeTruthy();
  });

  it("names the failing surface in the fallback subtitle", () => {
    render(
      <AppErrorBoundary surface="launcher">
        <Boom />
      </AppErrorBoundary>,
    );
    expect(screen.getByText(/The launcher window/i)).toBeTruthy();
  });

  it("copies error details to the clipboard when Copy details is clicked", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    render(
      <AppErrorBoundary surface="main">
        <Boom message="copy me" />
      </AppErrorBoundary>,
    );
    fireEvent.click(screen.getByRole("button", { name: /Copy details/i }));
    // Give the async clipboard write a tick to settle.
    await Promise.resolve();
    expect(writeText).toHaveBeenCalledTimes(1);
    expect(writeText.mock.calls[0][0]).toContain("Surface: main");
    expect(writeText.mock.calls[0][0]).toContain("Message: copy me");
  });
});
