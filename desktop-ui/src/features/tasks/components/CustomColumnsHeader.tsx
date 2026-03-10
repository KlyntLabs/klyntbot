import type { CustomColumn } from "@shared/types";

interface CustomColumnsHeaderProps {
  columns: CustomColumn[];
}

export function CustomColumnsHeader({ columns }: CustomColumnsHeaderProps) {
  return (
    <>
      {columns.map((col) => (
        <th
          key={col.id}
          className="px-5 py-2.5 font-light tracking-wide uppercase text-[11px] text-muted"
          style={col.width ? { width: col.width, minWidth: col.width } : undefined}
        >
          {col.name}
        </th>
      ))}
    </>
  );
}
