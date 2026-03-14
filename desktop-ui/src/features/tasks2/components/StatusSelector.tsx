import { useMutation } from "@shared/hooks/useMutation";
import type { Task, TaskUpdateParams } from "@shared/types/tasks";
import { Check } from "lucide-react";
import { useState } from "react";
import type { Status } from "../lib/mappers";
import { status as allStatus } from "../lib/status-icons";
import { renderStatusIcon } from "../lib/status-utils";
import { cn } from "../lib/utils";
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
}

export function StatusSelector({ issueId, status }: StatusSelectorProps) {
  const [open, setOpen] = useState(false);
  const updateTask = useMutation<Task, TaskUpdateParams>("task_update", "params");

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="flex items-center justify-center size-5 rounded hover:bg-[hsl(var(--accent))] transition-colors"
          aria-label={`Status: ${status.name}`}
        >
          {renderStatusIcon(status.id)}
        </button>
      </PopoverTrigger>
      <PopoverContent className="w-[200px] p-0" align="start">
        <Command>
          <CommandInput placeholder="Set status..." />
          <CommandList>
            <CommandEmpty>No status found.</CommandEmpty>
            <CommandGroup>
              {allStatus.map((s) => (
                <CommandItem
                  key={s.id}
                  value={s.name}
                  onSelect={() => {
                    updateTask.mutate({ id: issueId, statusLabelId: s.id });
                    setOpen(false);
                  }}
                >
                  <span className="mr-2 flex items-center">{renderStatusIcon(s.id)}</span>
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
