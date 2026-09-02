import { useMutation } from "@shared/hooks/useMutation";
import { cn } from "@shared/lib/utils";
import type { Task, TaskUpdateParams } from "@shared/types/tasks";
import { Check } from "lucide-react";
import { useState } from "react";
import { useStatusWorkflow } from "../contexts/StatusWorkflowContext";
import { useRefetchTasks } from "../hooks/useTasksContext";
import { statusToMutationParams } from "../lib/mappers";
import type { Status } from "../lib/status-icons";
import { renderStatusIcon } from "../lib/status-utils";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "./ui/command";
import { Popover, PopoverContent, PopoverTrigger } from "./ui/popover";

interface StatusSelectorProps {
  issueId: string;
  status: Status;
  onChanged?: () => void;
}

export function StatusSelector({ issueId, status, onChanged }: StatusSelectorProps) {
  const [open, setOpen] = useState(false);
  const { statuses } = useStatusWorkflow();
  const updateTask = useMutation<Task, TaskUpdateParams>("task_update", "params");
  const refetch = useRefetchTasks();

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="flex items-center justify-center size-5 rounded hover:bg-control-hover transition-colors"
          aria-label={`Status: ${status.name}`}
        >
          {renderStatusIcon(status)}
        </button>
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
                  onSelect={async () => {
                    const { status: backendStatus, statusLabelId } = statusToMutationParams(s);
                    await updateTask.mutate({ id: issueId, status: backendStatus, statusLabelId });
                    refetch();
                    onChanged?.();
                    setOpen(false);
                  }}
                >
                  <span className="mr-2 flex items-center">{renderStatusIcon(s)}</span>
                  {s.name}
                  <Check
                    className={cn(
                      "ml-auto h-4 w-4",
                      status.id === s.id ? "opacity-100" : "opacity-0",
                    )}
                  />
                </CommandItem>
              ))}
            </CommandGroup>
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}
