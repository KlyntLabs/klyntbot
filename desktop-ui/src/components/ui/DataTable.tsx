import { ChevronDown, ChevronRight } from "lucide-react";
import { type ReactNode, useCallback } from "react";
import { cn } from "../../lib/utils";

// ── Column Definition ────────────────────────────────────────────────

export interface DataTableColumn<T> {
  /** Unique key for this column */
  key: string;
  /** Header label (rendered uppercase) */
  header: string;
  /** Tailwind width class, e.g. "w-40" */
  width?: string;
  /** Text alignment — defaults to "left" */
  align?: "left" | "center" | "right";
  /** Render the cell content for a given row item */
  renderCell: (item: T) => ReactNode;
  /** Optional header className override */
  headerClassName?: string;
}

// ── Props ────────────────────────────────────────────────────────────

export interface DataTableProps<T> {
  columns: DataTableColumn<T>[];
  data: T[];
  /** Extract a unique key from each item */
  rowKey: (item: T) => string;
  /** Currently expanded row key (null = none) */
  expandedKey?: string | null;
  /**
   * Toggle expand on a row. When provided alongside `expandable`,
   * the entire row acts as an expand toggle. `onRowClick` is ignored
   * when this is set.
   */
  onToggleExpand?: (key: string) => void;
  /** Render the expanded detail below a row */
  renderExpanded?: (item: T) => ReactNode;
  /** Optional leading column content per row (e.g. toggle switch) */
  renderRowPrefix?: (item: T) => ReactNode;
  /** Called when a row is clicked. Ignored when onToggleExpand is set. */
  onRowClick?: (item: T) => void;
  /** Additional className per row */
  rowClassName?: (item: T) => string;
  /** Show loading skeleton */
  loading?: boolean;
  /** Number of skeleton rows to show */
  skeletonRows?: number;
  /** Empty state content */
  emptyState?: ReactNode;
  /** Whether to show the expand chevron column */
  expandable?: boolean;
  /** Outer wrapper className */
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
      {/* Static skeleton list — index keys are intentional */}
      {Array.from({ length: count }, (_, i) => (
        <tr key={`skel-${i}`} className="border-b border-white/[0.04]">
          {expandable && (
            <td className="px-3 py-2.5 w-8">
              <div className="w-3 h-3 rounded animate-pulse bg-white/[0.08]" />
            </td>
          )}
          {hasPrefix && (
            <td className="px-2 py-2.5 w-10">
              <div className="w-7 h-4 rounded-full animate-pulse bg-white/[0.08]" />
            </td>
          )}
          {columns.map((col) => (
            <td key={col.key} className={cn("px-5 py-2.5", col.width)}>
              <div className="h-4 rounded animate-pulse bg-white/[0.08]" />
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
          <tr className="border-b border-white/[0.06] text-[11px] text-muted font-light text-left bg-white/[0.03]">
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
          "transition-colors border-b border-white/[0.04] last:border-b-0 whitespace-nowrap",
          isInteractive && "hover:bg-white/[0.04] cursor-pointer",
          !isInteractive && "hover:bg-white/[0.02]",
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
        <tr className="border-b border-white/[0.04]">
          <td colSpan={totalCols} className="p-0">
            {renderExpanded(item)}
          </td>
        </tr>
      )}
    </>
  );
}
