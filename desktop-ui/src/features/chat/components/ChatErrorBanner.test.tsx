/**
 * @vitest-environment jsdom
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { fireEvent } from "@testing-library/react";
import { ChatErrorBanner } from "./ChatErrorBanner";

describe("ChatErrorBanner", () => {
  it("renders nothing when error is null", () => {
    const { container } = render(
      <ChatErrorBanner error={null} onDismiss={() => {}} />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("renders the error string when set", () => {
    render(<ChatErrorBanner error="boom" onDismiss={() => {}} />);
    expect(screen.getByRole("alert")).toHaveTextContent("boom");
  });

  it("invokes onDismiss when the dismiss button is clicked", () => {
    const onDismiss = vi.fn();
    const { container } = render(<ChatErrorBanner error="boom" onDismiss={onDismiss} />);
    const btn = container.querySelector(".chat-error-banner__dismiss");
    expect(btn).not.toBeNull();
    fireEvent.click(btn!);
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });
});
