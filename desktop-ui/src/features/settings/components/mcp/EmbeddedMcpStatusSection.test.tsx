import type { EmbeddedMcpStatusResponse } from "@shared/types";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { EmbeddedMcpStatusSection } from "./EmbeddedMcpStatusSection";

const ready: EmbeddedMcpStatusResponse = {
  state: "ready",
  requested: [],
  effective: ["get_status", "agent", "tasks"],
  rejected: [],
};

const disabled: EmbeddedMcpStatusResponse = {
  state: "disabled",
  requested: [],
  effective: ["get_status", "agent", "tasks"],
  rejected: [],
};

const invalid: EmbeddedMcpStatusResponse = {
  state: "invalid",
  requested: ["tasks", "bogus", "shell"],
  effective: ["get_status", "tasks"],
  rejected: [
    { name: "bogus", reason: "unknown" },
    { name: "shell", reason: "forbidden" },
  ],
};

describe("EmbeddedMcpStatusSection", () => {
  it("shows Ready chip and effective tool count", () => {
    render(<EmbeddedMcpStatusSection status={ready} />);
    expect(screen.getByRole("heading", { name: "Embedded MCP server" })).toBeInTheDocument();
    expect(screen.getByLabelText("Status: Ready")).toHaveTextContent("Ready");
    expect(screen.getByText("3 tools exposed")).toBeInTheDocument();
    expect(screen.queryByText(/Rejection summary/)).not.toBeInTheDocument();
  });

  it("shows Disabled chip without treating client servers as status", () => {
    render(<EmbeddedMcpStatusSection status={disabled} />);
    expect(screen.getByLabelText("Status: Disabled")).toHaveTextContent("Disabled");
    expect(screen.getByText("Server is off in configuration")).toBeInTheDocument();
    expect(screen.getByText(/in-process MCP server/i)).toBeInTheDocument();
  });

  it("shows Invalid chip and reachable rejection summary", () => {
    render(<EmbeddedMcpStatusSection status={invalid} />);
    expect(screen.getByLabelText("Status: Invalid")).toHaveTextContent("Invalid");
    expect(screen.getByText("Rejection summary (2)")).toBeInTheDocument();
    expect(screen.getByText("bogus")).toBeInTheDocument();
    expect(screen.getByText("unknown tool")).toBeInTheDocument();
    expect(screen.getByText("shell")).toBeInTheDocument();
    expect(screen.getByText("forbidden")).toBeInTheDocument();
  });

  it("renders loading skeleton when status is pending", () => {
    const { container } = render(<EmbeddedMcpStatusSection status={null} loading />);
    expect(screen.getByRole("heading", { name: "Embedded MCP server" })).toBeInTheDocument();
    expect(container.querySelector("[aria-busy='true']")).toBeInTheDocument();
  });
});
