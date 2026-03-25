import { cn } from "@shared/lib/utils";
import { ChevronDown, ChevronRight } from "lucide-react";
import { type ReactNode, useCallback } from "react";

// ── Column Definition ────────────────────────────────────────────────

export interface DataTableColumn<T> {
  key: string;
  header: string;
  width?: string;
  align?: "left" | "center" | "right";
  renderCell: (item: T) => ReactNode;
  headerClassName?: string;
}

// ── Props ────────────────────────────────────────────────────────────

export interface DataTableProps<T> {
  columns: DataTableColumn<T>[];
  data: T[];
  rowKey: (item: T) => string;
  expandedKey?: string | null;
  onToggleExpand?: (key: string) => void;
  renderExpanded?: (item: T) => ReactNode;
  renderRowPrefix?: (item: T) => ReactNode;
  onRowClick?: (item: T) => void;
  rowClassName?: (item: T) => string;
  loading?: boolean;
  skeletonRows?: number;
  emptyState?: ReactNode;
  expandable?: boolean;
  className?: string;
}

// ── Skeleton ─────────────────────────────────────────────────────────

function SkeletonRows<T>({
  columns,
  count,
  hasPrefix,
  expandable,
}: {
  columns: DataTableColumn<T>[];
  count: number;
  hasPrefix: boolean;
  expandable: boolean;
}) {
  return (
    <>
      {Array.from({ length: count }, (_, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: skeleton rows are static placeholders that never reorder
        <tr key={`skeleton-${i}`} className="border-b border-border-subtle">
          {expandable && (
            <td className="px-3 py-2.5 w-8">
              <div className="size-3 rounded animate-pulse bg-muted" />
            </td>
          )}
          {hasPrefix && (
            <td className="px-2 py-2.5 w-10">
              <div className="w-7 h-4 rounded-full animate-pulse bg-muted" />
            </td>
          )}
          {columns.map((col) => (
            <td key={col.key} className={cn("px-5 py-2.5", col.width)}>
              <div className="h-4 rounded animate-pulse bg-muted" />
            </td>
          ))}
        </tr>
      ))}
    </>
  );
}

// ── DataTable ────────────────────────────────────────────────────────

export function DataTable<T>({
  columns,
  data,
  rowKey,
  expandedKey = null,
  onToggleExpand,
  renderExpanded,
  renderRowPrefix,
  onRowClick,
  rowClassName,
  loading = false,
  skeletonRows = 6,
  emptyState,
  expandable = false,
  className,
}: DataTableProps<T>) {
  const totalCols = columns.length + (expandable ? 1 : 0) + (renderRowPrefix ? 1 : 0);

  return (
    <div className={cn("overflow-hidden", className)}>
      <table className="w-full border-collapse">
        <thead>
          <tr className="border-b border-border-subtle text-[11px] text-muted-foreground font-light text-left bg-card">
            {expandable && <th className="px-3 py-2.5 w-8 font-light" />}
            {renderRowPrefix && <th className="px-2 py-2.5 w-10 font-light" />}
            {columns.map((col) => (
              <th
                key={col.key}
                className={cn(
                  "px-5 py-2.5 font-light tracking-wide uppercase",
                  col.width,
                  col.align === "right" && "text-right",
                  col.align === "center" && "text-center",
                  col.headerClassName,
                )}
              >
                {col.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {loading ? (
            <SkeletonRows
              columns={columns}
              count={skeletonRows}
              hasPrefix={!!renderRowPrefix}
              expandable={expandable}
            />
          ) : data.length === 0 && emptyState ? (
            <tr>
              <td colSpan={totalCols}>{emptyState}</td>
            </tr>
          ) : (
            data.map((item) => {
              const key = rowKey(item);
              return (
                <DataTableRow
                  key={key}
                  item={item}
                  itemKey={key}
                  columns={columns}
                  isExpanded={expandedKey === key}
                  expandable={expandable}
                  onToggleExpand={onToggleExpand}
                  renderExpanded={renderExpanded}
                  renderRowPrefix={renderRowPrefix}
                  onRowClick={onRowClick}
                  rowClassName={rowClassName}
                  totalCols={totalCols}
                />
              );
            })
          )}
        </tbody>
      </table>
    </div>
  );
}

// ── Row ──────────────────────────────────────────────────────────────

function DataTableRow<T>({
  item,
  itemKey,
  columns,
  isExpanded,
  expandable,
  onToggleExpand,
  renderExpanded,
  renderRowPrefix,
  onRowClick,
  rowClassName,
  totalCols,
}: {
  item: T;
  itemKey: string;
  columns: DataTableColumn<T>[];
  isExpanded: boolean;
  expandable: boolean;
  onToggleExpand?: (key: string) => void;
  renderExpanded?: (item: T) => ReactNode;
  renderRowPrefix?: (item: T) => ReactNode;
  onRowClick?: (item: T) => void;
  rowClassName?: (item: T) => string;
  totalCols: number;
}) {
  const handleClick = useCallback(() => {
    if (onToggleExpand) onToggleExpand(itemKey);
    else if (onRowClick) onRowClick(item);
  }, [onToggleExpand, onRowClick, itemKey, item]);

  const isInteractive = !!onToggleExpand || !!onRowClick;

  return (
    <>
      <tr
        tabIndex={isInteractive ? 0 : undefined}
        onClick={isInteractive ? handleClick : undefined}
        onKeyDown={isInteractive ? (e) => e.key === "Enter" && handleClick() : undefined}
        className={cn(
          "transition-colors border-b border-border-subtle last:border-b-0 whitespace-nowrap",
          isInteractive && "hover:bg-accent cursor-pointer",
          !isInteractive && "hover:bg-card",
          rowClassName?.(item),
        )}
      >
        {expandable && (
          <td className="px-3 py-2.5 w-8">
            {isExpanded ? (
              <ChevronDown size={14} className="text-dim" strokeWidth={1.5} />
            ) : (
              <ChevronRight size={14} className="text-dim" strokeWidth={1.5} />
            )}
          </td>
        )}
        {renderRowPrefix && (
          <td
            className="px-2 py-2.5 w-10"
            onClick={(e) => e.stopPropagation()}
            onKeyDown={(e) => e.stopPropagation()}
          >
            {renderRowPrefix(item)}
          </td>
        )}
        {columns.map((col) => (
          <td
            key={col.key}
            className={cn(
              "px-5 py-2.5",
              col.width,
              col.align === "right" && "text-right",
              col.align === "center" && "text-center",
            )}
          >
            {col.renderCell(item)}
          </td>
        ))}
      </tr>
      {isExpanded && renderExpanded && (
        <tr className="border-b border-border-subtle">
          <td colSpan={totalCols} className="p-0">
            {renderExpanded(item)}
          </td>
        </tr>
      )}
    </>
  );
}
