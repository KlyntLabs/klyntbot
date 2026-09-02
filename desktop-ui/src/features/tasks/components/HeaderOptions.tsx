import type { Project } from "@shared/types/tasks";
import { Button } from "@shared/ui/Button";
import { Columns3, List } from "lucide-react";
import { useMemo } from "react";
import type { Issue } from "../lib/mappers";
import { projectToDisplayProject } from "../lib/mappers";
import { useViewStore } from "../store/view-store";
import { Filter } from "./Filter";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@shared/ui";

interface HeaderOptionsProps {
  issues: Issue[];
  projects: Project[];
}

export default function HeaderOptions({ issues, projects }: HeaderOptionsProps) {
  const { viewType, setViewType } = useViewStore();

  const displayProjects = useMemo(() => projects.map(projectToDisplayProject), [projects]);

  return (
    <div className="flex items-center justify-between px-4 py-1.5 border-b border-separator">
      {/* Left */}
      <div className="flex items-center gap-2">
        <Filter issues={issues} projects={displayProjects} />
      </div>

      {/* Right */}
      <div className="flex items-center gap-2">
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button size="xs" variant="ghost">
              {viewType === "list" ? (
                <List className="size-4 mr-1" />
              ) : (
                <Columns3 className="size-4 mr-1" />
              )}
              {viewType === "list" ? "List" : "Board"}
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem onSelect={() => setViewType("list")}>
              <List className="size-4 mr-2" />
              List
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => setViewType("grid")}>
              <Columns3 className="size-4 mr-2" />
              Board
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>
  );
}
