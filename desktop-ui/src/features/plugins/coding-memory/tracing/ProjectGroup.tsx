import type { ReactNode } from "react";
import { useState } from "react";

interface Props {
  basename: string;
  cwd: string;
  count: number;
  children: ReactNode;
  defaultOpen?: boolean;
}

export function ProjectGroup({ basename, cwd, count, children, defaultOpen = true }: Props) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div className="tracing-group">
      <button type="button" className="tracing-group__header" onClick={() => setOpen(!open)}>
        <span className="tracing-group__chevron" aria-hidden>
          {open ? "▾" : "▸"}
        </span>
        <span className="tracing-group__icon" aria-hidden>
          📁
        </span>
        <span className="tracing-group__name">{basename}</span>
        <span className="tracing-group__count">({count})</span>
        <span className="tracing-group__path">{cwd}</span>
      </button>
      {open && <div className="tracing-group__body">{children}</div>}
    </div>
  );
}
