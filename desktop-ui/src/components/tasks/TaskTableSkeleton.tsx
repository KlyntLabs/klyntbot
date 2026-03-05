const SKELETON_ROW_WIDTHS = [60, 73, 86, 67, 79] as const;
const EXTRA_CELLS_BASE = ["project", "priority", "status", "due", "tags"] as const;
const EXTRA_CELLS_WITH_AREA = ["project", "area", "priority", "status", "due", "tags"] as const;

export function TaskTableSkeleton({ showArea = false }: { showArea?: boolean }) {
  const extraCells = showArea ? EXTRA_CELLS_WITH_AREA : EXTRA_CELLS_BASE;
  return (
    <div className="mb-10 rounded-xl">
      <table className="w-full bg-surface-low border-collapse">
        <thead>
          <tr className="border-b border-border text-[11px] text-muted font-light text-left">
            <th className="px-5 py-3 w-9 font-light" />
            <th className="px-5 py-3 font-light">Task</th>
            <th className="px-5 py-3 font-light">Project</th>
            {showArea && <th className="px-5 py-3 font-light">Area</th>}
            <th className="px-5 py-3 font-light">Priority</th>
            <th className="px-5 py-3 font-light">Status</th>
            <th className="px-5 py-3 font-light">Due Date</th>
            <th className="px-5 py-3 font-light">Tags</th>
          </tr>
        </thead>
        <tbody>
          {(["a", "b", "c", "d", "e"] as const).map((k, i) => (
            <tr key={k} className="border-b border-border-subtle">
              <td className="px-5 py-3 w-9">
                <div className="w-4 h-4 rounded bg-surface-raised animate-pulse" />
              </td>
              <td className="px-5 py-3">
                <div
                  className="h-3 rounded bg-surface-raised animate-pulse"
                  style={{ width: `${SKELETON_ROW_WIDTHS[i]}%` }}
                />
              </td>
              {extraCells.map((col) => (
                <td key={col} className="px-5 py-3">
                  <div className="h-3 w-16 rounded bg-surface-raised animate-pulse" />
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
