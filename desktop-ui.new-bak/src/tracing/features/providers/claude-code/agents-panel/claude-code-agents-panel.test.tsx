import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ClaudeCodeAgentsPanel } from "./claude-code-agents-panel";

describe("ClaudeCodeAgentsPanel", () => {
  it("renders agent type and description", () => {
    render(
      <ClaudeCodeAgentsPanel
        agents={[
          {
            agent_id: "AAA",
            subagent_type: "code-reviewer",
            status: "completed",
            description: "Verify",
            created_at: 0,
            updated_at: 0,
            last_task_id: null,
            wire_size: 0,
            context_size: 0,
            launch_spec: {},
          },
        ]}
        onSelect={vi.fn()}
      />,
    );
    expect(screen.getByText("code-reviewer")).toBeInTheDocument();
    expect(screen.getByText("Verify")).toBeInTheDocument();
  });
});
