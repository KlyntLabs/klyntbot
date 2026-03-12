import { Columns3, List } from "lucide-react";
import { useViewStore } from "../store/view-store";
import { Filter } from "./Filter";
import { Button } from "./ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "./ui/dropdown-menu";

export default function HeaderOptions() {
  const { viewType, setViewType } = useViewStore();

  return (
    <div className="flex items-center justify-between px-4 py-1.5 border-b border-[hsl(var(--border))]">
      {/* Left */}
      <div className="flex items-center gap-2">
        <Filter />
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
