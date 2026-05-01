// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AppModeSwitch } from "./AppModeSwitch";

afterEach(() => cleanup());

describe("AppModeSwitch", () => {
  it("renders both modes with assistant active", () => {
    render(<AppModeSwitch mode="assistant" onChange={() => {}} />);
    const a = screen.getByRole("tab", { name: "Assistant" });
    const c = screen.getByRole("tab", { name: "Code" });
    expect(a.getAttribute("aria-selected")).toBe("true");
    expect(c.getAttribute("aria-selected")).toBe("false");
    expect(a.className).toContain("app-mode-switch__btn--active");
    expect(c.className).not.toContain("app-mode-switch__btn--active");
  });

  it("renders code as active when mode='code'", () => {
    render(<AppModeSwitch mode="code" onChange={() => {}} />);
    expect(screen.getByRole("tab", { name: "Code" }).getAttribute("aria-selected")).toBe("true");
    expect(screen.getByRole("tab", { name: "Assistant" }).getAttribute("aria-selected")).toBe(
      "false",
    );
  });

  it("calls onChange('code') when clicking the Code segment", () => {
    const onChange = vi.fn();
    render(<AppModeSwitch mode="assistant" onChange={onChange} />);
    fireEvent.click(screen.getByRole("tab", { name: "Code" }));
    expect(onChange).toHaveBeenCalledWith("code");
  });

  it("does not call onChange when clicking the active segment", () => {
    const onChange = vi.fn();
    render(<AppModeSwitch mode="assistant" onChange={onChange} />);
    fireEvent.click(screen.getByRole("tab", { name: "Assistant" }));
    expect(onChange).not.toHaveBeenCalled();
  });

  it("ArrowRight calls onChange with next mode", () => {
    const onChange = vi.fn();
    render(<AppModeSwitch mode="assistant" onChange={onChange} />);
    fireEvent.keyDown(screen.getByRole("tablist"), { key: "ArrowRight" });
    expect(onChange).toHaveBeenCalledWith("code");
  });

  it("ArrowLeft wraps from assistant to code", () => {
    const onChange = vi.fn();
    render(<AppModeSwitch mode="assistant" onChange={onChange} />);
    fireEvent.keyDown(screen.getByRole("tablist"), { key: "ArrowLeft" });
    expect(onChange).toHaveBeenCalledWith("code");
  });

  it("ArrowRight wraps from code to assistant", () => {
    const onChange = vi.fn();
    render(<AppModeSwitch mode="code" onChange={onChange} />);
    fireEvent.keyDown(screen.getByRole("tablist"), { key: "ArrowRight" });
    expect(onChange).toHaveBeenCalledWith("assistant");
  });

  it("sets data-tauri-drag-region=false on the wrapper and buttons", () => {
    render(<AppModeSwitch mode="assistant" onChange={() => {}} />);
    const list = screen.getByRole("tablist");
    expect(list.getAttribute("data-tauri-drag-region")).toBe("false");
    expect(screen.getByRole("tab", { name: "Code" }).getAttribute("data-tauri-drag-region")).toBe(
      "false",
    );
  });

  it("labels the tablist 'App mode'", () => {
    render(<AppModeSwitch mode="assistant" onChange={() => {}} />);
    expect(screen.getByRole("tablist").getAttribute("aria-label")).toBe("App mode");
  });
});
