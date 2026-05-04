// @vitest-environment jsdom
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ProviderChips } from "./ProviderChips";

describe("ProviderChips", () => {
  it("renders all five agent sources plus All", () => {
    render(<ProviderChips active="all" onChange={() => {}} />);
    expect(screen.getByRole("tab", { name: /^all$/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /claude code/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /codex/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /kimi/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /opencode/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /klynt cli/i })).toBeInTheDocument();
  });

  it("emits klyntCli when the Klynt CLI pill is clicked", () => {
    const onChange = vi.fn();
    render(<ProviderChips active="all" onChange={onChange} />);
    fireEvent.click(screen.getByRole("tab", { name: /klynt cli/i }));
    expect(onChange).toHaveBeenCalledWith("klyntCli");
  });

  it("marks the active pill with aria-selected=true", () => {
    render(<ProviderChips active="klyntCli" onChange={() => {}} />);
    expect(screen.getByRole("tab", { name: /klynt cli/i })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByRole("tab", { name: /^all$/i })).toHaveAttribute("aria-selected", "false");
  });
});
