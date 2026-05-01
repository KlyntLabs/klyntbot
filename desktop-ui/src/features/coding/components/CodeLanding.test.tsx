// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { CodeLanding, type CodeLandingProject } from "./CodeLanding";

afterEach(() => cleanup());

const sampleProjects: CodeLandingProject[] = [
  { id: "ws-1", name: "klyntbot", branch: "feature/code-mode", sessionCount: 12 },
  { id: "ws-2", name: "raki-web", branch: "main", sessionCount: 3 },
  { id: "ws-3", name: "desktop-ui", branch: null, sessionCount: 0 },
];

const baseHandlers = {
  onSelectProject: vi.fn(),
  onAddProject: vi.fn(),
  onImportProject: vi.fn(),
};

describe("CodeLanding (populated)", () => {
  it("renders the hero with the populated heading", () => {
    render(<CodeLanding projects={sampleProjects} {...baseHandlers} />);
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe(
      "What should we build today?",
    );
  });

  it("renders a card for each project with name, sessions, and branch", () => {
    render(<CodeLanding projects={sampleProjects} {...baseHandlers} />);
    expect(screen.getByText("klyntbot")).toBeTruthy();
    expect(screen.getByText("raki-web")).toBeTruthy();
    expect(screen.getByText("desktop-ui")).toBeTruthy();
    expect(screen.getByText("12 sessions")).toBeTruthy();
    expect(screen.getByText("3 sessions")).toBeTruthy();
    expect(screen.getByText("0 sessions")).toBeTruthy();
    expect(screen.getByText("feature/code-mode")).toBeTruthy();
    expect(screen.getByText("main")).toBeTruthy();
  });

  it("shows the project count in the section header", () => {
    render(<CodeLanding projects={sampleProjects} {...baseHandlers} />);
    const header = screen.getByRole("heading", { level: 2 });
    expect(header.textContent).toBe("Projects");
    expect(screen.getByText("3", { selector: ".count" })).toBeTruthy();
  });

  it("clicking a project card calls onSelectProject with its id", () => {
    const onSelectProject = vi.fn();
    render(
      <CodeLanding projects={sampleProjects} {...baseHandlers} onSelectProject={onSelectProject} />,
    );
    fireEvent.click(screen.getByText("klyntbot").closest("button") as HTMLButtonElement);
    expect(onSelectProject).toHaveBeenCalledWith("ws-1");
  });

  it("clicking the trailing + New project card calls onAddProject", () => {
    const onAddProject = vi.fn();
    render(<CodeLanding projects={sampleProjects} {...baseHandlers} onAddProject={onAddProject} />);
    fireEvent.click(screen.getByRole("button", { name: /new project/i }));
    expect(onAddProject).toHaveBeenCalled();
  });

  it("clicking the header Import button calls onImportProject", () => {
    const onImportProject = vi.fn();
    render(
      <CodeLanding projects={sampleProjects} {...baseHandlers} onImportProject={onImportProject} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Import" }));
    expect(onImportProject).toHaveBeenCalled();
  });

  it("clicking the header + New button also calls onAddProject", () => {
    const onAddProject = vi.fn();
    render(<CodeLanding projects={sampleProjects} {...baseHandlers} onAddProject={onAddProject} />);
    fireEvent.click(screen.getByRole("button", { name: "+ New" }));
    expect(onAddProject).toHaveBeenCalled();
  });

  it("renders no branch line when branch is null", () => {
    render(
      <CodeLanding
        projects={[{ id: "x", name: "no-branch", branch: null, sessionCount: 0 }]}
        {...baseHandlers}
      />,
    );
    expect(document.querySelector(".code-landing__card-branch")).toBeNull();
  });
});

describe("CodeLanding (empty)", () => {
  it("renders the Get started heading when there are no projects", () => {
    render(<CodeLanding projects={[]} {...baseHandlers} />);
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("Get started");
  });

  it("does not render the Projects section when empty", () => {
    render(<CodeLanding projects={[]} {...baseHandlers} />);
    expect(screen.queryByRole("heading", { level: 2 })).toBeNull();
    expect(document.querySelector(".code-landing__grid")).toBeNull();
  });

  it("renders both empty-state CTAs", () => {
    render(<CodeLanding projects={[]} {...baseHandlers} />);
    expect(screen.getByRole("button", { name: "Create first project" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Import existing repo" })).toBeTruthy();
  });

  it("Create first project calls onAddProject", () => {
    const onAddProject = vi.fn();
    render(<CodeLanding projects={[]} {...baseHandlers} onAddProject={onAddProject} />);
    fireEvent.click(screen.getByRole("button", { name: "Create first project" }));
    expect(onAddProject).toHaveBeenCalled();
  });

  it("Import existing repo calls onImportProject", () => {
    const onImportProject = vi.fn();
    render(<CodeLanding projects={[]} {...baseHandlers} onImportProject={onImportProject} />);
    fireEvent.click(screen.getByRole("button", { name: "Import existing repo" }));
    expect(onImportProject).toHaveBeenCalled();
  });

  it("does not render the descriptive subtitle in empty state", () => {
    render(<CodeLanding projects={[]} {...baseHandlers} />);
    expect(screen.queryByText(/Pick a project below or describe a task/)).toBeNull();
  });
});
