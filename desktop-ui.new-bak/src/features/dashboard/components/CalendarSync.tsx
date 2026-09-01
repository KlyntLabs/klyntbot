import Calendar from "lucide-react/dist/esm/icons/calendar";
import Loader2 from "lucide-react/dist/esm/icons/loader-2";
import RefreshCw from "lucide-react/dist/esm/icons/refresh-cw";
import { useState } from "react";
import { calendarSyncEvents } from "@/api/endpoints/dashboard";
import { useTauriMutation } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { formatTime } from "@/utils/dashboardDates";

export function CalendarSync() {
  const [lastSynced, setLastSynced] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const { mutate, isLoading } = useTauriMutation<unknown, void>({
    mutationFn: () => calendarSyncEvents(),
    invalidates: [qk.dashboard.all(), qk.calendarSync.all()],
    onSuccess: () => {
      setError(null);
      setLastSynced(formatTime(new Date().toISOString()));
    },
    onError: (e) => {
      setError(e instanceof Error ? e.message : "Sync failed");
    },
  });

  const title = lastSynced
    ? `Last synced: ${lastSynced}`
    : error
      ? `Error: ${error}`
      : "Sync calendar events";

  return (
    <button
      type="button"
      onClick={() => void mutate()}
      disabled={isLoading}
      className="dashboard__calendar-sync"
      title={title}
    >
      {isLoading ? <Loader2 className="lc-spin" /> : <Calendar />}
      <span>Sync</span>
      {lastSynced && !isLoading && <RefreshCw />}
    </button>
  );
}
