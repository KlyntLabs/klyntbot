import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("../hooks", () => ({
  useSensitivityInspector: () => ({
    data: [
      { id: "1", subject: "repo:bot", predicate: "api_key", object: "sk-***", sensitivity: "high" },
      { id: "2", subject: "repo:bot", predicate: "framework", object: "tauri", sensitivity: "normal" },
    ],
    isLoading: false,
  }),
}));

import { SensitivityInspectorPanel } from "../SensitivityInspectorPanel";

describe("SensitivityInspectorPanel", () => {
  it("renders sensitivity badges", () => {
    render(<SensitivityInspectorPanel />);
    expect(screen.getByText("Sensitivity Inspector")).toBeInTheDocument();
    expect(screen.getByText("repo:bot → api_key")).toBeInTheDocument();
    expect(screen.getByText("high")).toBeInTheDocument();
    expect(screen.getByText("repo:bot → framework")).toBeInTheDocument();
    expect(screen.getByText("normal")).toBeInTheDocument();
  });
});
