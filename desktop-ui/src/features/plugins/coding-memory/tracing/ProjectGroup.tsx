import { useState } from "react";
import type { ReactNode } from "react";

interface Props {
  basename: string;
  cwd: string;
  count: number;
  children: ReactNode;
  defaultOpen?: boolean;
  layout?: "grid" | "list";
}

export function ProjectGroup({ basename, cwd, count, children, defaultOpen = true, layout = "grid" }: Props) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div className="tracing-group">
      <button type="button" className="tracing-group__header" onClick={() => setOpen(!open)}>
        <span className="tracing-group__chevron" aria-hidden>{open ? "▾" : "▸"}</span>
        <span className="tracing-group__icon" aria-hidden>📁</span>
        <span className="tracing-group__name">{basename}</span>
        <span className="tracing-group__count">({count})</span>
        <span className="tracing-group__path">{cwd}</span>
      </button>
      {open && (
        <div className={layout === "list" ? "tracing-group__body tracing-group__body--list" : "tracing-group__body"}>
          {children}
        </div>
      )}
    </div>
  );
}
