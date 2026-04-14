import type { Entity, SortRule } from "@shared/types";
import { flexRender, type Header, type Table } from "@tanstack/react-table";

interface Props {
  table: Table<Entity>;
  sorts: SortRule[] | undefined;
  onSortChange: ((sorts: SortRule[]) => void) | undefined;
}

export function TableHeaderRow({ table, sorts, onSortChange }: Props) {
  const toggleSort = (slug: string) => {
    if (!onSortChange) return;
    const existing = sorts?.find((s) => s.field === slug);
    if (!existing) onSortChange([{ field: slug, direction: "asc" }]);
    else if (existing.direction === "asc") onSortChange([{ field: slug, direction: "desc" }]);
    else onSortChange([]);
  };

  const totalWidth = table.getTotalSize();

  const headers = table.getFlatHeaders();

  return (
    <div
      className="sticky top-0 z-10 flex border-b border-border bg-background text-[12px] font-medium text-foreground/70"
      style={{ minWidth: totalWidth, width: "100%" }}
    >
      {headers.map((header, i) => (
        <HeaderCell
          key={header.id}
          header={header}
          rule={sorts?.find((s) => s.field === header.id)}
          onToggle={() => toggleSort(header.id)}
          isLast={i === headers.length - 1}
        />
      ))}
    </div>
  );
}

function HeaderCell({
  header,
  rule,
  onToggle,
  isLast,
}: {
  header: Header<Entity, unknown>;
  rule: SortRule | undefined;
  onToggle: () => void;
  isLast: boolean;
}) {
  const size = header.getSize();
  return (
    <div
      className={`relative flex items-center ${isLast ? "" : "border-r border-border/40"}`}
      style={isLast ? { flex: 1, minWidth: size } : { width: size }}
    >
      <button
        type="button"
        onClick={onToggle}
        className="flex-1 truncate px-3 py-2 text-left cursor-pointer select-none transition-colors hover:bg-accent hover:text-foreground"
      >
        {flexRender(header.column.columnDef.header, header.getContext())}
        {rule && (
          <span
            className={`ml-1 text-[10px] ${rule.direction === "desc" ? "rotate-180 inline-block" : ""}`}
          >
            ▲
          </span>
        )}
      </button>
      <div
        onMouseDown={header.getResizeHandler()}
        onTouchStart={header.getResizeHandler()}
        className={`absolute right-0 top-0 h-full w-[4px] cursor-col-resize select-none touch-none ${
          header.column.getIsResizing() ? "bg-accent" : "hover:bg-accent/60"
        }`}
        aria-hidden="true"
      />
    </div>
  );
}
