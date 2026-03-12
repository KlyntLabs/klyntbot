import { Check } from "lucide-react";
import { useState } from "react";
import { cn } from "../lib/utils";
import type { Priority } from "../mock-data/priorities";
import { priorities } from "../mock-data/priorities";
import { useIssuesStore } from "../store/issues-store";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "./ui/command";
import { Popover, PopoverContent, PopoverTrigger } from "./ui/popover";

interface PrioritySelectorProps {
  issueId: string;
  priority: Priority;
}

export function PrioritySelector({ issueId, priority }: PrioritySelectorProps) {
  const [open, setOpen] = useState(false);
  const updateIssuePriority = useIssuesStore((s) => s.updateIssuePriority);

  const PriorityIcon = priority.icon;

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="flex items-center justify-center size-5 rounded hover:bg-[hsl(var(--accent))] transition-colors text-[hsl(var(--muted-foreground))]"
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
                    onSelect={() => {
                      updateIssuePriority(issueId, p);
                      setOpen(false);
                    }}
                  >
                    <Icon className="mr-2 h-4 w-4 text-[hsl(var(--muted-foreground))]" />
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
