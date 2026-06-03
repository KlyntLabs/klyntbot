export function LoadingState() {
  return (
    <div className="py-2" role="status" aria-busy="true">
      {[1, 2, 3, 4, 5, 6].map((i) => (
        <div key={`loading-${i}`} className="flex items-center gap-3 px-4 py-2">
          <div className="w-6 h-1 rounded bg-surface-control animate-[lc-skeleton-pulse_1.6s_ease-in-out_infinite]" />
          <div className="flex-1 flex flex-col gap-1.5">
            <div className="h-2.5 rounded bg-surface-control w-[70%] animate-[lc-skeleton-pulse_1.6s_ease-in-out_infinite]" />
            <div className="h-2.5 rounded bg-surface-control w-[40%] animate-[lc-skeleton-pulse_1.6s_ease-in-out_infinite]" />
          </div>
        </div>
      ))}
    </div>
  );
}
