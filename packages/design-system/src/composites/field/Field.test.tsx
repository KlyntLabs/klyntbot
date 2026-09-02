import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Field } from "./Field";

describe("Field", () => {
  it("associates the label with the control", () => {
    render(<Field id="title" label="Title" value="" onChange={() => {}} />);

    const input = screen.getByLabelText("Title");
    expect(input.tagName).toBe("INPUT");
    input.focus();
    expect(input).toHaveFocus();
  });

  it("announces description and error via aria-describedby", () => {
    render(
      <Field
        id="email"
        label="Email"
        description="Work address"
        value=""
        onChange={() => {}}
        error="Enter a valid email"
      />,
    );

    const input = screen.getByLabelText("Email");
    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent("Enter a valid email");
    expect(screen.getByText("Work address")).toBeInTheDocument();
    expect(input.getAttribute("aria-describedby")).toContain("email-description");
    expect(input.getAttribute("aria-describedby")).toContain("email-error");
  });

  it("sets aria-invalid only when in error", () => {
    const { rerender } = render(<Field id="name" label="Name" value="" onChange={() => {}} />);
    expect(screen.getByLabelText("Name")).not.toHaveAttribute("aria-invalid");

    rerender(
      <Field id="name" label="Name" value="" onChange={() => {}} error="Name is required" />,
    );
    expect(screen.getByLabelText("Name")).toHaveAttribute("aria-invalid", "true");
  });

  it("renders no alert or aria-describedby when idle", () => {
    render(<Field id="nickname" label="Nickname" value="" onChange={() => {}} />);

    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Nickname")).not.toHaveAttribute("aria-describedby");
  });

  it("renders a textarea when multiline is set", () => {
    render(<Field id="body" label="Body" multiline value="" onChange={() => {}} />);

    expect(screen.getByLabelText("Body").tagName).toBe("TEXTAREA");
  });

  it("applies pill shape when requested", () => {
    render(<Field id="pill" label="Pill" shape="pill" value="" onChange={() => {}} />);

    expect(screen.getByLabelText("Pill").className).toMatch(/\brounded-full\b/);
  });
});
