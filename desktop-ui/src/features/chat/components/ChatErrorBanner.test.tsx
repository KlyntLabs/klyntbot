/**
 * @vitest-environment jsdom
 */

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ChatErrorBanner } from "./ChatErrorBanner";

describe("ChatErrorBanner", () => {
  it("renders nothing when error is null", () => {
    const { container } = render(<ChatErrorBanner error={null} onDismiss={() => {}} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("renders the error string when set", () => {
    render(<ChatErrorBanner error="boom" onDismiss={() => {}} />);
    expect(screen.getByRole("alert")).toHaveTextContent("boom");
  });

  it("invokes onDismiss when the dismiss button is clicked", () => {
    const onDismiss = vi.fn();
    render(<ChatErrorBanner error="boom" onDismiss={onDismiss} />);
    const btn = screen.getByLabelText("Dismiss error");
    expect(btn).not.toBeNull();
    fireEvent.click(btn as HTMLElement);
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });
});
