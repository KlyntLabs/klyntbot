import { ActivityColumn } from "./timeline/ActivityColumn";
import { MemoriesColumn } from "./timeline/MemoriesColumn";
import { NotesColumn } from "./timeline/NotesColumn";
import { TasksColumn } from "./timeline/TasksColumn";

interface ProjectTimelineProps {
  projectId: string;
}

const COLUMNS = ["Activity", "Tasks", "Notes", "Memories"] as const;

export function ProjectTimeline({ projectId }: ProjectTimelineProps) {
  return (
    <div className="flex flex-col h-full">
      {/* Column headers */}
      <div className="grid grid-cols-4 border-b border-white/[0.06] shrink-0">
        {COLUMNS.map((col) => (
          <div
            key={col}
            className="px-3 py-2 text-[10px] font-medium text-dim uppercase tracking-wider"
          >
            {col}
          </div>
        ))}
      </div>

      {/* Column content */}
      <div className="grid grid-cols-4 flex-1 overflow-hidden">
        <div className="border-r border-white/[0.04] overflow-y-auto">
          <ActivityColumn projectId={projectId} />
        </div>
        <div className="border-r border-white/[0.04] overflow-y-auto">
          <TasksColumn projectId={projectId} />
        </div>
        <div className="border-r border-white/[0.04] overflow-y-auto">
          <NotesColumn projectId={projectId} />
        </div>
        <div className="overflow-y-auto">
          <MemoriesColumn projectId={projectId} />
        </div>
      </div>
    </div>
  );
}
