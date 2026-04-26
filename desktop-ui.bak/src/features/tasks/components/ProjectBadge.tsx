import type { DisplayProject } from "../lib/mappers";

export function ProjectBadge({ project }: { project: DisplayProject }) {
  return (
    <span className="inline-flex items-center gap-1.5 rounded-full border border-border px-2.5 py-0.5 text-xs text-muted-foreground bg-background">
      <project.icon size={16} />
      {project.name}
    </span>
  );
}
