import { render, screen } from "@testing-library/react";
import { ProgressRing } from "../ProgressRing";

describe("ProgressRing", () => {
  it("renders with correct progress percentage", () => {
    render(<ProgressRing progress={73} size="md" />);
    expect(screen.getByText("73%")).toBeInTheDocument();
  });

  it("renders small size without text", () => {
    const { container } = render(<ProgressRing progress={50} size="sm" />);
    expect(container.querySelector("svg")).toHaveAttribute("width", "28");
    expect(screen.queryByText("50%")).not.toBeInTheDocument();
  });

  it("applies gradient when gradient prop is true", () => {
    const { container } = render(<ProgressRing progress={80} size="md" gradient />);
    expect(container.querySelector("linearGradient")).toBeInTheDocument();
  });

  it("uses custom color", () => {
    const { container } = render(<ProgressRing progress={60} size="md" color="#f59e0b" />);
    const circle = container.querySelectorAll("circle")[1];
    expect(circle).toHaveAttribute("stroke", "#f59e0b");
  });
});
