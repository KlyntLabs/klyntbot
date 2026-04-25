import { useActivityTimeline } from "./hooks";

export function ActivityTimelinePanel() {
  const { data, isLoading } = useActivityTimeline();

  return (
    <div className="p-6">
      <h2 className="text-lg font-medium mb-4">Activity Timeline</h2>
      {isLoading && <div className="text-muted-foreground">Loading…</div>}
      {!isLoading && (!data || data.length === 0) && (
        <div className="text-muted-foreground">No activity yet.</div>
      )}
      <div className="grid grid-cols-7 gap-1">
        {data?.map((bucket) => (
          <div
            key={bucket.date}
            className="aspect-square rounded-md flex items-center justify-center text-xs"
            style={{
              backgroundColor: `rgba(59, 130, 246, ${Math.min(bucket.count / 10, 1)})`,
            }}
            title={`${bucket.date}: ${bucket.count} episodes`}
          >
            {bucket.count > 0 ? bucket.count : ""}
          </div>
        ))}
      </div>
    </div>
  );
}
