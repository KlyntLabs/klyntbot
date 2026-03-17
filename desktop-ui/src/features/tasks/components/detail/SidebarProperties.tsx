import { formatDate, formatHumanDuration } from "@shared/lib/dates";
import { Check } from "lucide-react";
import { useState } from "react";
import { useStatusWorkflow } from "../../contexts/StatusWorkflowContext";
import type { DetailTask } from "../../lib/mappers";
import { priorities } from "../../lib/priority-icons";
import { renderStatusIcon } from "../../lib/status-utils";
import { cn } from "../../lib/utils";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "../ui/command";
import { Popover, PopoverContent, PopoverTrigger } from "../ui/popover";

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
      <span className="w-[72px] shrink-0 text-xs text-muted-foreground">{label}</span>
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
        "flex items-center gap-1.5 px-1.5 py-0.5 rounded text-xs text-foreground hover:bg-accent transition-colors w-full text-left",
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
    ? formatDate(new Date(task.dueDate).toISOString().slice(0, 10))
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
              <PriorityIcon className="size-3.5 text-muted-foreground shrink-0" />
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
                        <Icon className="mr-2 h-4 w-4 text-muted-foreground" />
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

      {/* Due date */}
      <PropertyRow label="Due">
        <span
          className={cn(
            "px-1.5 py-0.5 text-xs",
            task.dueDate ? "text-foreground" : "text-muted-foreground",
          )}
        >
          {dueDateDisplay}
        </span>
      </PropertyRow>

      {/* Estimate */}
      <PropertyRow label="Estimate">
        <span
          className={cn(
            "px-1.5 py-0.5 text-xs",
            task.estimatedMinutes != null ? "text-foreground" : "text-muted-foreground",
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
            <span className="px-1.5 py-0.5 text-xs text-foreground">
              {task.area?.name ?? "No area"}
            </span>
          </PropertyRow>

          {/* Project */}
          <PropertyRow label="Project">
            <span
              className={cn(
                "px-1.5 py-0.5 text-xs",
                task.project ? "text-foreground" : "text-muted-foreground",
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
                    className="px-1.5 py-0.5 rounded-full bg-muted text-foreground text-xs"
                  >
                    {tag}
                  </span>
                ))}
              </div>
            ) : (
              <span className="px-1.5 py-0.5 text-xs text-muted-foreground">None</span>
            )}
          </PropertyRow>
        </>
      )}
    </div>
  );
}
