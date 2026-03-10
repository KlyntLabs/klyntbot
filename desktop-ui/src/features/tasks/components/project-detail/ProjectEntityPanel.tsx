import type { ReactNode } from "react";

interface ProjectEntityPanelProps {
  projectId: string;
  children?: ReactNode;
}

export function ProjectEntityPanel({ children }: ProjectEntityPanelProps) {
  return (
    <div className="w-72 border-l border-white/[0.06] overflow-y-auto shrink-0">
      <div className="py-2">{children}</div>
    </div>
  );
}
