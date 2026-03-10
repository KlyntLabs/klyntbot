/**
 * CalendarSync — toolbar button for syncing calendar events from Google Calendar MCP.
 * Shows sync status and triggers sync on click.
 */

import { useMutation } from "@shared/hooks/useMutation";
import { formatTime } from "@shared/lib/dates";
import { cn } from "@shared/lib/utils";
import { Calendar, Loader2, RefreshCw } from "lucide-react";
import { useState } from "react";

export function CalendarSync() {
  const { mutate, loading } = useMutation("calendar_sync_events");
  const [lastSynced, setLastSynced] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleSync = async () => {
    setError(null);
    try {
      // The frontend sends an empty array to trigger a "pull" sync
      // In production, this would be populated by MCP Google Calendar data
      await mutate({ events: [] });
      setLastSynced(formatTime(new Date().toISOString()));
    } catch (e) {
      setError(e instanceof Error ? e.message : "Sync failed");
    }
  };

  return (
    <button
      type="button"
      onClick={handleSync}
      disabled={loading}
      className={cn(
        "flex items-center gap-1.5 px-2 py-1 rounded-md text-xs",
        "text-muted hover:text-secondary hover:bg-surface-hover transition-colors",
        loading && "opacity-50 cursor-not-allowed",
      )}
      title={
        lastSynced
          ? `Last synced: ${lastSynced}`
          : error
            ? `Error: ${error}`
            : "Sync calendar events"
      }
    >
      {loading ? (
        <Loader2 className="w-3.5 h-3.5 animate-spin" />
      ) : (
        <Calendar className="w-3.5 h-3.5" />
      )}
      <span className="hidden sm:inline">Sync</span>
      {lastSynced && !loading && <RefreshCw className="w-3 h-3 text-success" />}
    </button>
  );
}
