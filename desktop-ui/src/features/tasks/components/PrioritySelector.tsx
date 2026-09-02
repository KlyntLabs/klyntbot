import { useMutation } from "@shared/hooks/useMutation";
import { cn } from "@shared/lib/utils";
import type { Task, TaskUpdateParams } from "@shared/types/tasks";
import { Check } from "lucide-react";
import { useState } from "react";
import { useRefetchTasks } from "../hooks/useTasksContext";
import type { Priority } from "../lib/mappers";
import { priorityToNumber } from "../lib/mappers";
import { priorities } from "../lib/priority-icons";
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

interface PrioritySelectorProps {
  issueId: string;
  priority: Priority;
  onChanged?: () => void;
}

export function PrioritySelector({ issueId, priority, onChanged }: PrioritySelectorProps) {
  const [open, setOpen] = useState(false);
  const updateTask = useMutation<Task, TaskUpdateParams>("task_update", "params");
  const refetch = useRefetchTasks();

  const PriorityIcon = priority.icon;

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="flex items-center justify-center size-5 rounded hover:bg-control-hover transition-colors text-fg-secondary"
          aria-label={`Priority: ${priority.name}`}
        >
          <PriorityIcon className="size-4" />
        </button>
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
                    onSelect={async () => {
                      await updateTask.mutate({ id: issueId, priority: priorityToNumber(p.id) });
                      refetch();
                      onChanged?.();
                      setOpen(false);
                    }}
                  >
                    <Icon className="mr-2 h-4 w-4 text-fg-secondary" />
                    {p.name}
                    <Check
                      className={cn(
                        "ml-auto h-4 w-4",
                        priority.id === p.id ? "opacity-100" : "opacity-0",
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
  );
}
