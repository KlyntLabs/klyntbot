import type { Area, Project } from "@shared/types/tasks";
import { Plus } from "lucide-react";
import { useState } from "react";
import { useTabStore } from "../store/tab-store";
import { Popover, PopoverContent, PopoverTrigger } from "./ui/popover";

const menuItemCls =
  "w-full text-left px-2 py-1.5 text-[13px] rounded-sm hover:bg-[hsl(var(--accent))] text-[hsl(var(--foreground))] transition-colors";

interface AddTabMenuProps {
  areas: Area[];
  projects: Project[];
}

export function AddTabMenu({ areas, projects }: AddTabMenuProps) {
  const openTab = useTabStore((s) => s.openTab);
  const [open, setOpen] = useState(false);

  const handleOpenArea = (area: Area) => {
    openTab("area", area.id, area.name);
    setOpen(false);
  };

  const handleOpenProject = (project: Project) => {
    openTab("project", project.id, project.name);
    setOpen(false);
  };

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="flex items-center justify-center w-[26px] h-[26px] rounded-md text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))] hover:bg-[hsl(var(--accent))] transition-colors flex-shrink-0"
        >
          <Plus className="h-4 w-4" />
        </button>
      </PopoverTrigger>
      <PopoverContent align="start" className="w-56 p-2">
        <button
          type="button"
          onClick={() => {
            openTab("all-issues", "all-issues", "All Issues");
            setOpen(false);
          }}
          className={`${menuItemCls} font-medium`}
        >
          All Issues
        </button>
        <div className="h-px bg-[hsl(var(--border))] my-1.5" />
        <div className="text-[11px] font-medium text-[hsl(var(--muted-foreground))] uppercase tracking-wider px-2 py-1">
          Areas
        </div>
        {areas.map((area) => (
          <button
            key={area.id}
            type="button"
            onClick={() => handleOpenArea(area)}
            className={menuItemCls}
          >
            {area.name}
          </button>
        ))}
        <div className="h-px bg-[hsl(var(--border))] my-1.5" />
        <div className="text-[11px] font-medium text-[hsl(var(--muted-foreground))] uppercase tracking-wider px-2 py-1">
          Projects
        </div>
        {projects.map((project) => (
          <button
            key={project.id}
            type="button"
            onClick={() => handleOpenProject(project)}
            className={menuItemCls}
          >
            {project.name}
          </button>
        ))}
      </PopoverContent>
    </Popover>
  );
}
