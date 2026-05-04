import ArrowUp from "lucide-react/dist/esm/icons/arrow-up";
import FolderPlus from "lucide-react/dist/esm/icons/folder-plus";
import { useState } from "react";

export type CodeLandingProject = {
  id: string;
  name: string;
  branch: string | null;
  sessionCount: number;
};

export type CodeLandingProps = {
  projects: CodeLandingProject[];
  onSelectProject: (id: string) => void;
  onAddProject: () => void;
  onImportProject: () => void;
};

export function CodeLanding({
  projects,
  onSelectProject,
  onAddProject,
  onImportProject,
}: CodeLandingProps) {
  const [draft, setDraft] = useState("");
  const isEmpty = projects.length === 0;

  return (
    <div className="code-landing">
      <div className="code-landing__inner">
        <div className="code-landing__hero">
          <h1>{isEmpty ? "Get started" : "What should we build today?"}</h1>
          {!isEmpty && <p>Pick a project below or describe a task to start a new session.</p>}
          <label className="code-landing__input">
            <input
              type="text"
              placeholder="Describe a task…"
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              aria-label="Describe a task"
            />
            <button
              type="button"
              className="code-landing__input-send"
              aria-label="Send task"
              disabled={draft.trim().length === 0}
            >
              <ArrowUp size={14} aria-hidden />
            </button>
          </label>
        </div>

        {isEmpty ? (
          <div className="code-landing__empty">
            <p>No projects yet. Create one or import an existing repo to begin.</p>
            <div className="code-landing__empty-actions">
              <button type="button" className="code-landing__empty-primary" onClick={onAddProject}>
                Create first project
              </button>
              <button
                type="button"
                className="code-landing__empty-secondary"
                onClick={onImportProject}
              >
                Import existing repo
              </button>
            </div>
          </div>
        ) : (
          <section className="code-landing__projects">
            <div className="code-landing__projects-header">
              <h2>Projects</h2>
              <span className="count">{projects.length}</span>
              <div className="code-landing__projects-actions">
                <button type="button" className="code-landing__ghost-btn" onClick={onImportProject}>
                  Import
                </button>
                <button type="button" className="code-landing__ghost-btn" onClick={onAddProject}>
                  + New
                </button>
              </div>
            </div>
            <div className="code-landing__grid">
              {projects.map((p) => (
                <button
                  key={p.id}
                  type="button"
                  className="code-landing__card"
                  onClick={() => onSelectProject(p.id)}
                >
                  <span className="code-landing__card-name">
                    <FolderPlus size={14} aria-hidden />
                    <span>{p.name}</span>
                  </span>
                  <span className="code-landing__card-meta">
                    {p.sessionCount === 1 ? "1 session" : `${p.sessionCount} sessions`}
                  </span>
                  {p.branch && <span className="code-landing__card-branch">{p.branch}</span>}
                </button>
              ))}
              <button
                type="button"
                className="code-landing__card code-landing__card--add"
                onClick={onAddProject}
                aria-label="New project"
              >
                <span className="plus" aria-hidden>
                  +
                </span>
                <span>New project</span>
              </button>
            </div>
          </section>
        )}
      </div>
    </div>
  );
}
