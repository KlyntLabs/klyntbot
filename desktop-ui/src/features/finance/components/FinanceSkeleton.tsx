export function FinanceSkeleton({ rows = 4 }: { rows?: number }) {
  return (
    <div className="space-y-4 animate-pulse">
      <div className="grid grid-cols-4 gap-4">
        {Array.from({ length: 4 }, (_, i) => (
          <div
            // biome-ignore lint/suspicious/noArrayIndexKey: skeleton placeholder from Array.from
            key={`skeleton-stat-${i}`}
            className="glass-card p-4 h-20 rounded-xl"
          />
        ))}
      </div>
      {Array.from({ length: rows }, (_, i) => (
        <div
          // biome-ignore lint/suspicious/noArrayIndexKey: skeleton placeholder from Array.from
          key={`skeleton-row-${i}`}
          className="glass-card p-4 h-16 rounded-xl"
        />
      ))}
    </div>
  );
}
