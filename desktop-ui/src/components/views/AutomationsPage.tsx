import {
  ChevronDown,
  ChevronRight,
  Clock,
  Filter,
  Play,
  Plus,
  Search,
  Trash2,
  X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useMutation } from "../../hooks/useMutation";
import { invalidateQueries, useQuery } from "../../hooks/useQuery";
import { humanizeJobName, humanizeSchedule, ORIGIN_STYLES, relativeTime } from "../../lib/cron";
import type { CronJob, CronJobCreateParams, CronOrigin, CronSchedule } from "../../lib/types";
import { cn } from "../../lib/utils";

type OriginFilter = "all" | CronOrigin;

// ── Origin Badge ────────────────────────────────────────────────────────

function OriginBadge({ origin }: { origin: CronOrigin }) {
  const style = ORIGIN_STYLES[origin];
  return (
    <span
      className={cn(
        "px-1.5 py-0.5 rounded text-[10px] font-medium uppercase tracking-wide",
        style.className,
      )}
    >
      {style.label}
    </span>
  );
}

// ── Expanded Row ────────────────────────────────────────────────────────

function AutomationExpandedRow({
  job,
  onRun,
  onDelete,
}: {
  job: CronJob;
  onRun: () => void;
  onDelete: () => void;
}) {
  const isSystem = job.origin === "system";

  return (
    <div className="px-4 py-3 bg-white/[0.02] border-t border-white/[0.06] space-y-3 text-xs">
      <div className="grid grid-cols-2 gap-x-6 gap-y-2">
        <div>
          <span className="text-[10px] text-dim uppercase tracking-wide">Schedule</span>
          <p className="text-muted font-mono text-[11px] mt-0.5">
            {humanizeSchedule(job.schedule)}
          </p>
        </div>
        <div>
          <span className="text-[10px] text-dim uppercase tracking-wide">Message</span>
          <p className="text-muted text-[12px] font-light mt-0.5">{job.payload.message || "—"}</p>
        </div>
        <div>
          <span className="text-[10px] text-dim uppercase tracking-wide">Delivery</span>
          <p className="text-muted text-[12px] font-light mt-0.5">
            {job.payload.deliver
              ? `${job.payload.channel ?? "?"} → ${job.payload.to ?? "?"}`
              : "Internal"}
          </p>
        </div>
        <div>
          <span className="text-[10px] text-dim uppercase tracking-wide">Last Run</span>
          <p className="text-muted text-[12px] font-light mt-0.5">
            {job.state.lastRunAtMs ? relativeTime(job.state.lastRunAtMs) : "Never"}
            {job.state.lastStatus && (
              <span
                className={cn(
                  "ml-1",
                  job.state.lastStatus === "ok" ? "text-success" : "text-destructive",
                )}
              >
                ({job.state.lastStatus})
              </span>
            )}
          </p>
        </div>
        {job.state.lastError && (
          <div className="col-span-2">
            <span className="text-[10px] text-dim uppercase tracking-wide">Last Error</span>
            <p className="text-destructive font-mono text-[11px] mt-0.5">{job.state.lastError}</p>
          </div>
        )}
      </div>

      <div className="flex gap-2 pt-1">
        <button
          type="button"
          onClick={onRun}
          className="glass-button px-2.5 py-1 text-[11px] flex items-center gap-1"
        >
          <Play size={11} /> Run Now
        </button>
        {!isSystem && (
          <button
            type="button"
            onClick={onDelete}
            className="glass-button px-2.5 py-1 text-[11px] flex items-center gap-1 text-destructive hover:bg-red-500/10"
          >
            <Trash2 size={11} /> Delete
          </button>
        )}
      </div>
    </div>
  );
}

// ── Job Row ─────────────────────────────────────────────────────────────

function AutomationRow({
  job,
  expanded,
  onToggleExpand,
  onEnable,
  onRun,
  onDelete,
}: {
  job: CronJob;
  expanded: boolean;
  onToggleExpand: () => void;
  onEnable: (enabled: boolean) => void;
  onRun: () => void;
  onDelete: () => void;
}) {
  return (
    <div className="border-b border-white/[0.04] last:border-b-0">
      <div
        role="button"
        tabIndex={0}
        onClick={onToggleExpand}
        onKeyDown={(e) => e.key === "Enter" && onToggleExpand()}
        className="w-full flex items-center gap-3 px-4 py-2.5 hover:bg-white/[0.06] transition-colors text-left cursor-pointer"
      >
        {expanded ? (
          <ChevronDown size={14} className="text-dim shrink-0" />
        ) : (
          <ChevronRight size={14} className="text-dim shrink-0" />
        )}

        {/* Toggle */}
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onEnable(!job.enabled);
          }}
          className={cn(
            "w-7 h-4 rounded-full shrink-0 transition-colors relative",
            job.enabled ? "bg-brand" : "bg-white/10",
          )}
        >
          <span
            className={cn(
              "absolute top-0.5 w-3 h-3 rounded-full bg-white transition-transform",
              job.enabled ? "left-3.5" : "left-0.5",
            )}
          />
        </button>

        {/* Name + Origin */}
        <div className="flex items-center gap-2 min-w-0 flex-1">
          <span
            className={cn(
              "text-[12px] font-light truncate",
              job.enabled ? "text-secondary" : "text-dim",
            )}
          >
            {humanizeJobName(job.name)}
          </span>
          <OriginBadge origin={job.origin} />
          {job.deleteAfterRun && <span className="text-[10px] text-dim italic">one-shot</span>}
        </div>

        {/* Schedule */}
        <span className="text-[12px] font-light text-muted shrink-0 w-40 text-right">
          {humanizeSchedule(job.schedule)}
        </span>

        {/* Last run */}
        <span className="text-[12px] font-light text-dim shrink-0 w-32 text-right">
          {job.state.lastRunAtMs ? relativeTime(job.state.lastRunAtMs) : "—"}
        </span>

        {/* Next run */}
        <span className="text-[12px] font-light text-dim shrink-0 w-28 text-right">
          {job.enabled && job.state.nextRunAtMs ? relativeTime(job.state.nextRunAtMs) : "—"}
        </span>
      </div>

      {expanded && <AutomationExpandedRow job={job} onRun={onRun} onDelete={onDelete} />}
    </div>
  );
}

// ── Schedule Builder ────────────────────────────────────────────────────

function JobScheduleBuilder({
  value,
  onChange,
}: {
  value: CronSchedule;
  onChange: (s: CronSchedule) => void;
}) {
  return (
    <div className="space-y-2">
      <div className="flex gap-2">
        {(["every", "cron", "at"] as const).map((kind) => (
          <button
            key={kind}
            type="button"
            onClick={() => {
              if (kind === "every") onChange({ kind: "every", everyMs: 3600000 });
              else if (kind === "cron") onChange({ kind: "cron", expr: "0 9 * * *" });
              else onChange({ kind: "at", atMs: Date.now() + 3600000 });
            }}
            className={cn(
              "px-2 py-1 text-xs rounded",
              value.kind === kind ? "glass-button-active" : "glass-button",
            )}
          >
            {kind === "every" ? "Interval" : kind === "cron" ? "Cron" : "One-time"}
          </button>
        ))}
      </div>

      {value.kind === "every" && (
        <div className="flex items-center gap-2">
          <span className="text-xs text-dim">Every</span>
          <input
            type="number"
            min={1}
            className="glass-input w-20 text-xs"
            value={Math.round(value.everyMs / 60000)}
            onChange={(e) =>
              onChange({
                kind: "every",
                everyMs: Number(e.target.value) * 60000,
              })
            }
          />
          <span className="text-xs text-dim">minutes</span>
        </div>
      )}

      {value.kind === "cron" && (
        <input
          type="text"
          className="glass-input w-full text-xs font-mono"
          value={value.expr}
          placeholder="0 9 * * *"
          onChange={(e) => onChange({ kind: "cron", expr: e.target.value, tz: value.tz })}
        />
      )}

      {value.kind === "at" && (
        <input
          type="datetime-local"
          className="glass-input w-full text-xs"
          value={new Date(value.atMs).toISOString().slice(0, 16)}
          onChange={(e) => onChange({ kind: "at", atMs: new Date(e.target.value).getTime() })}
        />
      )}
    </div>
  );
}

// ── Create Form ─────────────────────────────────────────────────────────

function AutomationCreateForm({
  onClose,
  onCreated,
}: {
  onClose: () => void;
  onCreated: () => void;
}) {
  const [name, setName] = useState("");
  const [message, setMessage] = useState("");
  const [schedule, setSchedule] = useState<CronSchedule>({
    kind: "every",
    everyMs: 3600000,
  });
  const { mutate, loading } = useMutation<CronJob, CronJobCreateParams>("cron_create", "params");

  const handleSubmit = async () => {
    if (!name.trim() || !message.trim()) return;
    const result = await mutate({ name, schedule, message });
    if (result) {
      invalidateQueries("cron_");
      onCreated();
      onClose();
    }
  };

  return (
    <div className="border border-white/[0.08] rounded-xl p-4 bg-white/[0.04] space-y-3 mx-4 mt-3">
      <div className="flex items-center justify-between">
        <h3 className="text-[13px] font-medium text-secondary">New Automation</h3>
        <button type="button" onClick={onClose} className="text-dim hover:text-muted">
          <X size={14} />
        </button>
      </div>

      <div className="space-y-2">
        <input
          type="text"
          className="glass-input w-full text-sm"
          placeholder="Job name"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <textarea
          className="glass-input w-full text-sm min-h-[60px] resize-none"
          placeholder="Agent message / prompt"
          value={message}
          onChange={(e) => setMessage(e.target.value)}
        />
        <JobScheduleBuilder value={schedule} onChange={setSchedule} />
      </div>

      <div className="flex justify-end gap-2">
        <button type="button" onClick={onClose} className="glass-button px-3 py-1.5 text-xs">
          Cancel
        </button>
        <button
          type="button"
          onClick={handleSubmit}
          disabled={loading || !name.trim() || !message.trim()}
          className="bg-brand hover:bg-brand-hover text-white px-3 py-1.5 rounded-xl text-[12px] font-light disabled:opacity-40 transition-colors"
        >
          {loading ? "Creating…" : "Create"}
        </button>
      </div>
    </div>
  );
}

// ── Loading Skeleton ────────────────────────────────────────────────────

function AutomationsSkeleton() {
  return (
    <div className="space-y-0">
      {Array.from({ length: 6 }).map((_, i) => (
        <div
          key={`skel-${i}`}
          className="flex items-center gap-3 px-4 py-3 border-b border-white/[0.04]"
        >
          <div className="w-4 h-4 rounded animate-pulse bg-white/[0.08]" />
          <div className="w-7 h-4 rounded-full animate-pulse bg-white/[0.08]" />
          <div className="flex-1 h-4 rounded animate-pulse bg-white/[0.08]" />
          <div className="w-28 h-4 rounded animate-pulse bg-white/[0.08]" />
          <div className="w-32 h-4 rounded animate-pulse bg-white/[0.08]" />
          <div className="w-24 h-4 rounded animate-pulse bg-white/[0.08]" />
        </div>
      ))}
    </div>
  );
}

// ── Main Page ───────────────────────────────────────────────────────────

export function AutomationsPage() {
  const {
    data: jobs,
    loading,
    refetch,
  } = useQuery<CronJob[]>("cron_list", { includeDisabled: true }, []);

  const { mutate: enableJob } = useMutation<CronJob, { id: string; enabled: boolean }>(
    "cron_enable",
  );
  const { mutate: runJob } = useMutation<boolean, { id: string }>("cron_run");
  const { mutate: deleteJob } = useMutation<boolean, { id: string }>("cron_delete");

  const [originFilter, setOriginFilter] = useState<OriginFilter>("all");
  const [searchQ, setSearchQ] = useState("");
  const [debouncedQ, setDebouncedQ] = useState("");
  const debounceRef = useRef<ReturnType<typeof setTimeout>>();
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);

  const handleSearch = (value: string) => {
    setSearchQ(value);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => setDebouncedQ(value), 300);
  };

  useEffect(
    () => () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    },
    [],
  );

  const filtered = useMemo(() => {
    let list = jobs ?? [];
    if (originFilter !== "all") {
      list = list.filter((j) => j.origin === originFilter);
    }
    if (debouncedQ) {
      const q = debouncedQ.toLowerCase();
      list = list.filter(
        (j) => j.name.toLowerCase().includes(q) || j.payload.message.toLowerCase().includes(q),
      );
    }
    return list;
  }, [jobs, originFilter, debouncedQ]);

  const handleEnable = useCallback(
    async (id: string, enabled: boolean) => {
      await enableJob({ id, enabled });
      invalidateQueries("cron_");
      refetch();
    },
    [enableJob, refetch],
  );

  const handleRun = useCallback(
    async (id: string) => {
      await runJob({ id });
      invalidateQueries("cron_");
      refetch();
    },
    [runJob, refetch],
  );

  const handleDelete = useCallback(
    async (id: string) => {
      await deleteJob({ id });
      invalidateQueries("cron_");
      refetch();
    },
    [deleteJob, refetch],
  );

  const originTabs: { key: OriginFilter; label: string }[] = [
    { key: "all", label: "All" },
    { key: "system", label: "System" },
    { key: "ai", label: "AI" },
    { key: "user", label: "User" },
    { key: "plugin", label: "Plugin" },
  ];

  return (
    <div className="flex-1 flex flex-col gap-2 overflow-hidden">
      {/* Floating glass toolbar — matches Tasks / Finance layout */}
      <div className="h-12 flex items-center px-2 shrink-0">
        <div className="flex-1 flex items-center gap-1.5">
          {originTabs.map((tab) => (
            <button
              key={tab.key}
              type="button"
              onClick={() => setOriginFilter(tab.key)}
              className={cn(
                "px-3 py-1.5 rounded-lg text-[12px] font-light transition-colors",
                originFilter === tab.key
                  ? "bg-white/[0.12] text-primary"
                  : "text-muted hover:text-secondary",
              )}
            >
              {tab.label}
            </button>
          ))}

          <div className="w-px h-5 bg-white/[0.08] mx-1" />

          {/* Search */}
          <div className="relative">
            <Search
              size={13}
              className="absolute left-2.5 top-1/2 -translate-y-1/2 text-dim"
              strokeWidth={1.5}
            />
            <input
              type="text"
              className="glass-input pl-8 pr-3 py-1.5 text-[12px] font-light w-48 rounded-lg"
              placeholder="Search automations…"
              value={searchQ}
              onChange={(e) => handleSearch(e.target.value)}
            />
          </div>
        </div>

        {/* Primary action — matches Tasks "+ Add task" style */}
        <button
          type="button"
          onClick={() => setShowCreate(true)}
          className="bg-brand hover:bg-brand-hover text-white px-3 py-1.5 rounded-xl text-[12px] font-light flex items-center gap-2 transition-colors"
        >
          <Plus className="w-[14px] h-[14px]" strokeWidth={1.5} /> Add automation
        </button>
      </div>

      {/* Content card */}
      <div className="flex-1 flex flex-col overflow-hidden rounded-xl border border-white/[0.06] bg-surface-raised">
        {/* Create Form */}
        {showCreate && (
          <AutomationCreateForm onClose={() => setShowCreate(false)} onCreated={refetch} />
        )}

        {/* Table Header */}
        <div className="flex items-center gap-3 px-4 py-2 text-[10px] uppercase tracking-wider text-dim border-b border-white/[0.06] bg-white/[0.02] shrink-0">
          <span className="w-4" />
          <span className="w-7" />
          <span className="flex-1">Name</span>
          <span className="w-40 text-right">Schedule</span>
          <span className="w-32 text-right">Last Run</span>
          <span className="w-28 text-right">Next Run</span>
        </div>

        {/* Job List */}
        <div className="flex-1 overflow-y-auto">
          {loading ? (
            <AutomationsSkeleton />
          ) : filtered.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-16 text-dim">
              <Clock size={32} className="mb-3 opacity-30" />
              <p className="text-muted text-sm font-light">
                {debouncedQ || originFilter !== "all"
                  ? "No matching automations"
                  : "No automations yet"}
              </p>
              <p className="text-dim text-xs font-light mt-1">
                {debouncedQ || originFilter !== "all"
                  ? "Try adjusting your filters"
                  : "Create an automation to get started"}
              </p>
              {!showCreate && originFilter === "all" && !debouncedQ && (
                <button
                  type="button"
                  onClick={() => setShowCreate(true)}
                  className="bg-brand hover:bg-brand-hover text-white mt-4 px-3 py-1.5 rounded-xl text-[12px] font-light flex items-center gap-2 transition-colors"
                >
                  <Plus className="w-[14px] h-[14px]" strokeWidth={1.5} /> Create your first
                  automation
                </button>
              )}
            </div>
          ) : (
            filtered.map((job) => (
              <AutomationRow
                key={job.id}
                job={job}
                expanded={expandedId === job.id}
                onToggleExpand={() => setExpandedId(expandedId === job.id ? null : job.id)}
                onEnable={(enabled) => handleEnable(job.id, enabled)}
                onRun={() => handleRun(job.id)}
                onDelete={() => handleDelete(job.id)}
              />
            ))
          )}
        </div>
      </div>
    </div>
  );
}
