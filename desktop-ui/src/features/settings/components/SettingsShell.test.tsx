import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { SettingsShell } from "./SettingsShell";

describe("SettingsShell", () => {
  it("renders nav from registry and switches content", () => {
    const registry = [{ id: "demo", label: "Demo", Component: () => <div>DEMO BODY</div> }];
    render(<SettingsShell domains={registry} />);
    expect(screen.getByText("Demo")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Demo"));
    expect(screen.getByText("DEMO BODY")).toBeInTheDocument();
  });

  it("switches between multiple domains", () => {
    const registry = [
      { id: "a", label: "Alpha", Component: () => <div>ALPHA</div> },
      { id: "b", label: "Beta", Component: () => <div>BETA</div> },
    ];
    render(<SettingsShell domains={registry} />);

    // Defaults to first domain
    expect(screen.getByText("ALPHA")).toBeInTheDocument();

    // Switch to second
    fireEvent.click(screen.getByText("Beta"));
    expect(screen.getByText("BETA")).toBeInTheDocument();
    expect(screen.queryByText("ALPHA")).not.toBeInTheDocument();
  });

  it("shows placeholder when domains is empty", () => {
    render(<SettingsShell domains={[]} />);
    expect(screen.getByText("Select a setting from the sidebar.")).toBeInTheDocument();
  });
});
