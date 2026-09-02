import { formatDate, formatHumanDuration, formatTime } from "@shared/lib/dates";
import { cn } from "@shared/lib/utils";
import { Check } from "lucide-react";
import { useState } from "react";
import { useStatusWorkflow } from "../../contexts/StatusWorkflowContext";
import type { DetailTask } from "../../lib/mappers";
import { priorities } from "../../lib/priority-icons";
import { renderStatusIcon } from "../../lib/status-utils";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@shared/ui";

type EnergyLevel = "low" | "medium" | "high" | "deep";
type TaskType = "manual" | "agentic" | "hybrid";

interface SidebarPropertiesProps {
  task: DetailTask;
  compact: boolean;
  onUpdate: <K extends keyof DetailTask>(field: K, value: DetailTask[K]) => void;
}

const ENERGY_LEVELS: EnergyLevel[] = ["low", "medium", "high", "deep"];
const TASK_TYPES: TaskType[] = ["manual", "agentic", "hybrid"];

function capitalize(str: string): string {
  return str.charAt(0).toUpperCase() + str.slice(1);
}

interface PropertyRowProps {
  label: string;
  children: React.ReactNode;
}

function PropertyRow({ label, children }: PropertyRowProps) {
  return (
    <div className="flex items-center gap-2 py-1.5">
      <span className="w-[72px] shrink-0 text-ui-sm text-fg-secondary">{label}</span>
      <div className="flex-1 min-w-0">{children}</div>
    </div>
  );
}

interface ValueButtonProps {
  children: React.ReactNode;
  onClick?: () => void;
  className?: string;
}

function ValueButton({ children, onClick, className }: ValueButtonProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "flex items-center gap-1.5 px-1.5 py-0.5 rounded text-ui-sm text-fg hover:bg-control-hover transition-colors w-full text-left",
        className,
      )}
    >
      {children}
    </button>
  );
}

export function SidebarProperties({ task, compact, onUpdate }: SidebarPropertiesProps) {
  const { statuses } = useStatusWorkflow();
  const [statusOpen, setStatusOpen] = useState(false);
  const [priorityOpen, setPriorityOpen] = useState(false);
  const [energyOpen, setEnergyOpen] = useState(false);
  const [typeOpen, setTypeOpen] = useState(false);

  const PriorityIcon = task.priority.icon;

  const dueDateDisplay = task.dueDate
    ? (() => {
        const d = new Date(task.dueDate);
        const dateStr = formatDate(d.toISOString().slice(0, 10));
        const hasTime = d.getHours() !== 0 || d.getMinutes() !== 0;
        return hasTime ? `${dateStr}, ${formatTime(task.dueDate)}` : dateStr;
      })()
    : "No due date";

  const estimateDisplay =
    task.estimatedMinutes != null ? formatHumanDuration(task.estimatedMinutes * 60) : "No estimate";

  return (
    <div className="px-4 py-3 space-y-0.5">
      {/* Status */}
      <PropertyRow label="Status">
        <Popover open={statusOpen} onOpenChange={setStatusOpen}>
          <PopoverTrigger asChild>
            <ValueButton>
              <span className="flex items-center shrink-0">{renderStatusIcon(task.status)}</span>
              <span className="truncate">{task.status.name}</span>
            </ValueButton>
          </PopoverTrigger>
          <PopoverContent className="w-[200px] p-0" align="start">
            <Command>
              <CommandInput placeholder="Set status..." />
              <CommandList>
                <CommandEmpty>No status found.</CommandEmpty>
                <CommandGroup>
                  {statuses.map((s) => (
                    <CommandItem
                      key={s.id}
                      value={s.name}
                      onSelect={() => {
                        onUpdate("status", s);
                        setStatusOpen(false);
                      }}
                    >
                      <span className="mr-2 flex items-center">{renderStatusIcon(s)}</span>
                      {s.name}
                      <Check
                        className={cn(
                          "ml-auto h-4 w-4",
                          task.status.id === s.id ? "opacity-100" : "opacity-0",
                        )}
                      />
                    </CommandItem>
                  ))}
                </CommandGroup>
              </CommandList>
            </Command>
          </PopoverContent>
        </Popover>
      </PropertyRow>

      {/* Priority */}
      <PropertyRow label="Priority">
        <Popover open={priorityOpen} onOpenChange={setPriorityOpen}>
          <PopoverTrigger asChild>
            <ValueButton>
              <PriorityIcon className="size-3.5 text-fg-secondary shrink-0" />
              <span className="truncate">{task.priority.name}</span>
            </ValueButton>
          </PopoverTrigger>
          <PopoverContent className="w-[200px] p-0" align="start">
            <Command>
              <CommandInput placeholder="Set priority..." />
              <CommandList>
                <CommandEmpty>No priority found.</CommandEmpty>
                <CommandGroup>
                  {priorities.map((p) => {
                    const Icon = p.icon;
                    return (
                      <CommandItem
                        key={p.id}
                        value={p.name}
                        onSelect={() => {
                          onUpdate("priority", p);
                          setPriorityOpen(false);
                        }}
                      >
                        <Icon className="mr-2 h-4 w-4 text-fg-secondary" />
                        {p.name}
                        <Check
                          className={cn(
                            "ml-auto h-4 w-4",
                            task.priority.id === p.id ? "opacity-100" : "opacity-0",
                          )}
                        />
                      </CommandItem>
                    );
                  })}
                </CommandGroup>
              </CommandList>
            </Command>
          </PopoverContent>
        </Popover>
      </PropertyRow>

      {/* Energy */}
      <PropertyRow label="Energy">
        <Popover open={energyOpen} onOpenChange={setEnergyOpen}>
          <PopoverTrigger asChild>
            <ValueButton>
              <span className="truncate">
                {task.energyLevel ? capitalize(task.energyLevel) : "None"}
              </span>
            </ValueButton>
          </PopoverTrigger>
          <PopoverContent className="w-[180px] p-0" align="start">
            <Command>
              <CommandInput placeholder="Set energy..." />
              <CommandList>
                <CommandEmpty>No energy level found.</CommandEmpty>
                <CommandGroup>
                  {ENERGY_LEVELS.map((level) => (
                    <CommandItem
                      key={level}
                      value={level}
                      onSelect={() => {
                        onUpdate("energyLevel", level);
                        setEnergyOpen(false);
                      }}
                    >
                      {capitalize(level)}
                      <Check
                        className={cn(
                          "ml-auto h-4 w-4",
                          task.energyLevel === level ? "opacity-100" : "opacity-0",
                        )}
                      />
                    </CommandItem>
                  ))}
                </CommandGroup>
              </CommandList>
            </Command>
          </PopoverContent>
        </Popover>
      </PropertyRow>

      {/* Complexity */}
      {task.complexityScore != null && (
        <PropertyRow label="Complexity">
          <ComplexityBadge score={task.complexityScore} />
        </PropertyRow>
      )}

      {/* Due date */}
      <PropertyRow label="Due">
        <span
          className={cn(
            "px-1.5 py-0.5 text-ui-sm",
            task.dueDate ? "text-fg" : "text-fg-secondary",
          )}
        >
          {dueDateDisplay}
        </span>
      </PropertyRow>

      {/* Estimate */}
      <PropertyRow label="Estimate">
        <span
          className={cn(
            "px-1.5 py-0.5 text-ui-sm",
            task.estimatedMinutes != null ? "text-fg" : "text-fg-secondary",
          )}
        >
          {estimateDisplay}
        </span>
      </PropertyRow>

      {/* Fields hidden in compact mode */}
      {!compact && (
        <>
          {/* Type */}
          <PropertyRow label="Type">
            <Popover open={typeOpen} onOpenChange={setTypeOpen}>
              <PopoverTrigger asChild>
                <ValueButton>
                  <span className="truncate">{capitalize(task.taskType)}</span>
                </ValueButton>
              </PopoverTrigger>
              <PopoverContent className="w-[180px] p-0" align="start">
                <Command>
                  <CommandInput placeholder="Set type..." />
                  <CommandList>
                    <CommandEmpty>No type found.</CommandEmpty>
                    <CommandGroup>
                      {TASK_TYPES.map((type) => (
                        <CommandItem
                          key={type}
                          value={type}
                          onSelect={() => {
                            onUpdate("taskType", type);
                            setTypeOpen(false);
                          }}
                        >
                          {capitalize(type)}
                          <Check
                            className={cn(
                              "ml-auto h-4 w-4",
                              task.taskType === type ? "opacity-100" : "opacity-0",
                            )}
                          />
                        </CommandItem>
                      ))}
                    </CommandGroup>
                  </CommandList>
                </Command>
              </PopoverContent>
            </Popover>
          </PropertyRow>

          {/* Area */}
          <PropertyRow label="Area">
            <span className="px-1.5 py-0.5 text-ui-sm text-fg">
              {task.area?.name ?? "No area"}
            </span>
          </PropertyRow>

          {/* Project */}
          <PropertyRow label="Project">
            <span
              className={cn(
                "px-1.5 py-0.5 text-ui-sm",
                task.project ? "text-fg" : "text-fg-secondary",
              )}
            >
              {task.project ? task.project.name : "No project"}
            </span>
          </PropertyRow>

          {/* Tags */}
          <PropertyRow label="Tags">
            {task.tags.length > 0 ? (
              <div className="flex flex-wrap gap-1 px-1.5 py-0.5">
                {task.tags.map((tag) => (
                  <span
                    key={tag}
                    className="px-1.5 py-0.5 rounded-full bg-control-hover text-fg text-ui-sm"
                  >
                    {tag}
                  </span>
                ))}
              </div>
            ) : (
              <span className="px-1.5 py-0.5 text-ui-sm text-fg-secondary">None</span>
            )}
          </PropertyRow>
        </>
      )}
    </div>
  );
}

function ComplexityBadge({ score }: { score: number }) {
  const { label, color } =
    score <= 30
      ? { label: "Low", color: "text-green-400 bg-green-500/20" }
      : score <= 60
        ? { label: "Medium", color: "text-yellow-400 bg-yellow-500/20" }
        : score <= 80
          ? { label: "High", color: "text-orange-400 bg-orange-500/20" }
          : { label: "Very High", color: "text-red-400 bg-red-500/20" };

  return (
    <span className={`text-ui-sm px-1.5 py-0.5 rounded ${color}`}>
      {label} ({score})
    </span>
  );
}
