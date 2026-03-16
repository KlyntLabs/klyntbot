import type { DisplayProject } from "../lib/mappers";

export function ProjectBadge({ project }: { project: DisplayProject }) {
  return (
    <span className="inline-flex items-center gap-1.5 rounded-full border border-[hsl(var(--border))] px-2.5 py-0.5 text-xs text-[hsl(var(--muted-foreground))] bg-[hsl(var(--background))]">
      <project.icon size={16} />
      {project.name}
    </span>
  );
}
