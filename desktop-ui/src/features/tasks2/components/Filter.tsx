import { Check, ChevronLeft, ChevronRight, ListFilter, X } from "lucide-react";
import { useMemo, useState } from "react";
import { renderStatusIcon } from "../lib/status-utils";
import { labels } from "../mock-data/labels";
import { priorities } from "../mock-data/priorities";
import { projects } from "../mock-data/projects";
import { status as allStatus } from "../mock-data/status";
import { users } from "../mock-data/users";
import { useFilterStore } from "../store/filter-store";
import { useIssuesStore } from "../store/issues-store";
import { Avatar, AvatarFallback, AvatarImage } from "./ui/avatar";
import { Button } from "./ui/button";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from "./ui/command";
import { Popover, PopoverContent, PopoverTrigger } from "./ui/popover";

type FilterCategory = "status" | "assignee" | "priority" | "labels" | "project";

const categories: { key: FilterCategory; label: string }[] = [
  { key: "status", label: "Status" },
  { key: "assignee", label: "Assignee" },
  { key: "priority", label: "Priority" },
  { key: "labels", label: "Labels" },
  { key: "project", label: "Project" },
];

export function Filter() {
  const [open, setOpen] = useState(false);
  const [activeCategory, setActiveCategory] = useState<FilterCategory | null>(null);

  const { filters, toggleFilter, clearFilters } = useFilterStore();
  const isFiltered = useFilterStore((s) => Object.values(s.filters).some((a) => a.length > 0));
  const activeCount = useFilterStore((s) =>
    Object.values(s.filters).reduce((sum, a) => sum + a.length, 0),
  );
  const issues = useIssuesStore((s) => s.issues);

  // Single-pass count maps instead of N*K filterBy* calls
  const counts = useMemo(() => {
    const status: Record<string, number> = {};
    const priority: Record<string, number> = {};
    const assignee: Record<string, number> = {};
    const label: Record<string, number> = {};
    const project: Record<string, number> = {};
    for (const issue of issues) {
      status[issue.status.id] = (status[issue.status.id] ?? 0) + 1;
      priority[issue.priority.id] = (priority[issue.priority.id] ?? 0) + 1;
      assignee[issue.assignee?.id ?? "unassigned"] =
        (assignee[issue.assignee?.id ?? "unassigned"] ?? 0) + 1;
      for (const l of issue.labels) {
        label[l.id] = (label[l.id] ?? 0) + 1;
      }
      if (issue.project) {
        project[issue.project.id] = (project[issue.project.id] ?? 0) + 1;
      }
    }
    return { status, priority, assignee, label, project };
  }, [issues]);

  function getCountForCategory(category: FilterCategory): number {
    return filters[category].length;
  }

  function renderCategoryList() {
    return (
      <Command>
        <CommandInput placeholder="Filter by..." />
        <CommandList>
          <CommandEmpty>No filter found.</CommandEmpty>
          <CommandGroup>
            {categories.map((cat) => {
              const count = getCountForCategory(cat.key);
              return (
                <CommandItem
                  key={cat.key}
                  onSelect={() => setActiveCategory(cat.key)}
                  className="flex items-center justify-between"
                >
                  <span>{cat.label}</span>
                  <div className="flex items-center gap-1">
                    {count > 0 && (
                      <span className="flex items-center justify-center size-5 rounded-full bg-[hsl(var(--primary))] text-[hsl(var(--primary-foreground))] text-xs">
                        {count}
                      </span>
                    )}
                    <ChevronRight className="size-4 text-[hsl(var(--muted-foreground))]" />
                  </div>
                </CommandItem>
              );
            })}
          </CommandGroup>
          {isFiltered && (
            <>
              <CommandSeparator />
              <CommandGroup>
                <CommandItem
                  onSelect={() => {
                    clearFilters();
                  }}
                  className="justify-center text-[hsl(var(--destructive))]"
                >
                  <X className="mr-2 size-4" />
                  Clear all filters
                </CommandItem>
              </CommandGroup>
            </>
          )}
        </CommandList>
      </Command>
    );
  }

  function renderStatusItems() {
    return (
      <Command>
        <CommandInput placeholder="Search status..." />
        <CommandList>
          <CommandEmpty>No status found.</CommandEmpty>
          <CommandGroup>
            {allStatus.map((s) => {
              const isSelected = filters.status.includes(s.id);
              const count = counts.status[s.id] ?? 0;
              return (
                <CommandItem
                  key={s.id}
                  onSelect={() => toggleFilter("status", s.id)}
                  className="flex items-center gap-2"
                >
                  <div className="flex items-center justify-center size-4">
                    {isSelected ? <Check className="size-4" /> : <span className="size-4" />}
                  </div>
                  <span className="flex items-center">{renderStatusIcon(s.id)}</span>
                  <span className="flex-1">{s.name}</span>
                  <span className="text-xs text-[hsl(var(--muted-foreground))]">{count}</span>
                </CommandItem>
              );
            })}
          </CommandGroup>
        </CommandList>
      </Command>
    );
  }

  function renderAssigneeItems() {
    return (
      <Command>
        <CommandInput placeholder="Search assignee..." />
        <CommandList>
          <CommandEmpty>No assignee found.</CommandEmpty>
          <CommandGroup>
            {users.map((user) => {
              const isSelected = filters.assignee.includes(user.id);
              const count = counts.assignee[user.id] ?? 0;
              return (
                <CommandItem
                  key={user.id}
                  onSelect={() => toggleFilter("assignee", user.id)}
                  className="flex items-center gap-2"
                >
                  <div className="flex items-center justify-center size-4">
                    {isSelected ? <Check className="size-4" /> : <span className="size-4" />}
                  </div>
                  <Avatar className="size-5">
                    <AvatarImage src={user.avatarUrl} alt={user.name} />
                    <AvatarFallback className="text-[10px]">
                      {user.name
                        .split(" ")
                        .map((n) => n[0])
                        .join("")}
                    </AvatarFallback>
                  </Avatar>
                  <span className="flex-1">{user.name}</span>
                  <span className="text-xs text-[hsl(var(--muted-foreground))]">{count}</span>
                </CommandItem>
              );
            })}
          </CommandGroup>
        </CommandList>
      </Command>
    );
  }

  function renderPriorityItems() {
    return (
      <Command>
        <CommandInput placeholder="Search priority..." />
        <CommandList>
          <CommandEmpty>No priority found.</CommandEmpty>
          <CommandGroup>
            {priorities.map((p) => {
              const isSelected = filters.priority.includes(p.id);
              const count = counts.priority[p.id] ?? 0;
              const Icon = p.icon;
              return (
                <CommandItem
                  key={p.id}
                  onSelect={() => toggleFilter("priority", p.id)}
                  className="flex items-center gap-2"
                >
                  <div className="flex items-center justify-center size-4">
                    {isSelected ? <Check className="size-4" /> : <span className="size-4" />}
                  </div>
                  <Icon className="size-4 text-[hsl(var(--muted-foreground))]" />
                  <span className="flex-1">{p.name}</span>
                  <span className="text-xs text-[hsl(var(--muted-foreground))]">{count}</span>
                </CommandItem>
              );
            })}
          </CommandGroup>
        </CommandList>
      </Command>
    );
  }

  function renderLabelItems() {
    return (
      <Command>
        <CommandInput placeholder="Search labels..." />
        <CommandList>
          <CommandEmpty>No labels found.</CommandEmpty>
          <CommandGroup>
            {labels.map((label) => {
              const isSelected = filters.labels.includes(label.id);
              const count = counts.label[label.id] ?? 0;
              return (
                <CommandItem
                  key={label.id}
                  onSelect={() => toggleFilter("labels", label.id)}
                  className="flex items-center gap-2"
                >
                  <div className="flex items-center justify-center size-4">
                    {isSelected ? <Check className="size-4" /> : <span className="size-4" />}
                  </div>
                  <span className="size-2 rounded-full" style={{ backgroundColor: label.color }} />
                  <span className="flex-1">{label.name}</span>
                  <span className="text-xs text-[hsl(var(--muted-foreground))]">{count}</span>
                </CommandItem>
              );
            })}
          </CommandGroup>
        </CommandList>
      </Command>
    );
  }

  function renderProjectItems() {
    return (
      <Command>
        <CommandInput placeholder="Search projects..." />
        <CommandList>
          <CommandEmpty>No projects found.</CommandEmpty>
          <CommandGroup>
            {projects.map((project) => {
              const isSelected = filters.project.includes(project.id);
              const count = counts.project[project.id] ?? 0;
              const Icon = project.icon;
              return (
                <CommandItem
                  key={project.id}
                  onSelect={() => toggleFilter("project", project.id)}
                  className="flex items-center gap-2"
                >
                  <div className="flex items-center justify-center size-4">
                    {isSelected ? <Check className="size-4" /> : <span className="size-4" />}
                  </div>
                  <Icon className="size-4 text-[hsl(var(--muted-foreground))]" />
                  <span className="flex-1">{project.name}</span>
                  <span className="text-xs text-[hsl(var(--muted-foreground))]">{count}</span>
                </CommandItem>
              );
            })}
          </CommandGroup>
        </CommandList>
      </Command>
    );
  }

  function renderCategoryItems() {
    switch (activeCategory) {
      case "status":
        return renderStatusItems();
      case "assignee":
        return renderAssigneeItems();
      case "priority":
        return renderPriorityItems();
      case "labels":
        return renderLabelItems();
      case "project":
        return renderProjectItems();
      default:
        return null;
    }
  }

  return (
    <Popover
      open={open}
      onOpenChange={(val) => {
        setOpen(val);
        if (!val) setActiveCategory(null);
      }}
    >
      <PopoverTrigger asChild>
        <Button size="xs" variant="ghost">
          <ListFilter className="size-4 mr-1" />
          Filter
          {activeCount > 0 && (
            <span className="ml-1 flex items-center justify-center size-5 rounded-full bg-[hsl(var(--primary))] text-[hsl(var(--primary-foreground))] text-xs">
              {activeCount}
            </span>
          )}
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-[240px] p-0" align="start">
        {activeCategory ? (
          <div>
            <button
              type="button"
              onClick={() => setActiveCategory(null)}
              className="flex items-center gap-1 px-3 py-2 text-sm text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))] transition-colors w-full border-b border-[hsl(var(--border))]"
            >
              <ChevronLeft className="size-4" />
              {categories.find((c) => c.key === activeCategory)?.label}
            </button>
            {renderCategoryItems()}
          </div>
        ) : (
          renderCategoryList()
        )}
      </PopoverContent>
    </Popover>
  );
}
