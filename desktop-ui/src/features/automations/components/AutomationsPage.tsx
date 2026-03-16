import { MiniCalendar } from "@shared/components/MiniCalendar";
import { DataTable, type DataTableColumn } from "@shared/composites/DataTable";
import { useMutation } from "@shared/hooks/useMutation";
import { invalidateQueries, useQuery } from "@shared/hooks/useQuery";
import { cn } from "@shared/lib/cn";
import { humanizeJobName, humanizeSchedule, ORIGIN_STYLES, relativeTime } from "@shared/lib/cron";
import { toLocalDateTime, toLocalISO } from "@shared/lib/dates";
import type {
  CronJob,
  CronJobCreateParams,
  CronJobUpdateParams,
  CronOrigin,
  CronSchedule,
} from "@shared/types";
import { Toggle } from "@shared/ui";
import { Clock, Play, Plus, Search, Trash2, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";

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

// ── Schedule Type Badge ─────────────────────────────────────────────────

function ScheduleTypeBadge({ schedule }: { schedule: CronSchedule }) {
  const label = schedule.kind === "every" ? "Interval" : schedule.kind === "cron" ? "Cron" : "Once";
  return (
    <span className="px-1.5 py-0.5 rounded text-[10px] font-light text-dim bg-white/[0.06]">
      {label}
    </span>
  );
}

// ── Inline Text Cell ────────────────────────────────────────────────────

function InlineTextCell({
  value,
  onSave,
  editable,
  className,
  placeholder,
}: {
  value: string;
  onSave: (v: string) => void;
  editable: boolean;
  className?: string;
  placeholder?: string;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");

  const startEdit = (e: React.SyntheticEvent) => {
    if (!editable) return;
    e.stopPropagation();
    setDraft(value);
    setEditing(true);
  };

  const save = () => {
    if (draft.trim() && draft !== value) onSave(draft.trim());
    setEditing(false);
  };

  if (editing) {
    return (
      <input
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === "Enter") save();
          if (e.key === "Escape") setEditing(false);
        }}
        onBlur={save}
        onClick={(e) => e.stopPropagation()}
        placeholder={placeholder}
        className={cn(
          "bg-transparent border-b border-brand outline-none w-full text-[12px] font-light text-primary",
          className,
        )}
      />
    );
  }

  return (
    <button
      type="button"
      onClick={startEdit}
      className={cn(
        "text-left truncate max-w-full",
        editable && "cursor-text rounded px-1 -mx-1 transition-colors hover:bg-white/[0.06]",
        className,
      )}
    >
      {value || <span className="text-dim">{placeholder ?? "—"}</span>}
    </button>
  );
}

// ── Schedule Helpers ─────────────────────────────────────────────────────

type ScheduleMode = "once" | "interval" | "cron";
type IntervalUnit = "minutes" | "hours" | "days" | "weeks";

const WEEKDAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] as const;
const WEEKDAY_CRON = [1, 2, 3, 4, 5, 6, 0]; // cron: 0=Sun, 1=Mon...

interface ScheduleFields {
  mode: ScheduleMode;
  intervalValue: number;
  intervalUnit: IntervalUnit;
  weekdays: boolean[];
  time: string;
  date: string;
  dateTime: string;
}

const DEFAULT_FIELDS: ScheduleFields = {
  mode: "interval",
  intervalValue: 1,
  intervalUnit: "hours",
  weekdays: [true, true, true, true, true, false, false],
  time: "09:00",
  date: toLocalISO(new Date()),
  dateTime: toLocalDateTime(new Date(Date.now() + 3600000)),
};

function parseSchedule(s: CronSchedule): ScheduleFields {
  if (s.kind === "at") {
    const d = new Date(s.atMs);
    return {
      ...DEFAULT_FIELDS,
      mode: "once",
      date: toLocalISO(d),
      time: `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`,
      dateTime: toLocalDateTime(d),
    };
  }

  if (s.kind === "every") {
    const ms = s.everyMs;
    let unit: IntervalUnit = "minutes";
    let val = Math.round(ms / 60000);
    if (ms >= 604800000 && ms % 604800000 === 0) {
      unit = "weeks";
      val = ms / 604800000;
    } else if (ms >= 86400000 && ms % 86400000 === 0) {
      unit = "days";
      val = ms / 86400000;
    } else if (ms >= 3600000 && ms % 3600000 === 0) {
      unit = "hours";
      val = ms / 3600000;
    }
    return { ...DEFAULT_FIELDS, mode: "interval", intervalValue: val, intervalUnit: unit };
  }

  // cron
  const parts = s.expr.split(" ");
  const minute = parts[0] ?? "0";
  const hour = parts[1] ?? "9";
  const dow = parts[4] ?? "*";
  const time = `${hour.padStart(2, "0")}:${minute.padStart(2, "0")}`;

  if (dow !== "*") {
    const activeDays = dow.split(",").map(Number);
    const weekdays = WEEKDAY_CRON.map((d) => activeDays.includes(d));
    return { ...DEFAULT_FIELDS, mode: "cron", time, weekdays, intervalUnit: "weeks" };
  }
  return { ...DEFAULT_FIELDS, mode: "cron", time, intervalUnit: "days" };
}

function fieldsToSchedule(f: ScheduleFields): CronSchedule {
  if (f.mode === "once") {
    return { kind: "at", atMs: new Date(`${f.date}T${f.time}`).getTime() };
  }
  if (f.mode === "interval") {
    const mult =
      f.intervalUnit === "weeks"
        ? 604800000
        : f.intervalUnit === "days"
          ? 86400000
          : f.intervalUnit === "hours"
            ? 3600000
            : 60000;
    return { kind: "every", everyMs: Math.max(1, f.intervalValue) * mult };
  }
  // cron
  const [h, m] = f.time.split(":").map(Number);
  if (f.intervalUnit === "weeks") {
    const days = WEEKDAY_CRON.filter((_, i) => f.weekdays[i]);
    const dowStr = days.length > 0 ? days.join(",") : "*";
    return { kind: "cron", expr: `${m} ${h} * * ${dowStr}` };
  }
  return { kind: "cron", expr: `${m} ${h} * * *` };
}

// ── Schedule Panel (shared by inline editor and create form) ────────────

function SchedulePanel({
  fields,
  onChange,
}: {
  fields: ScheduleFields;
  onChange: (f: ScheduleFields) => void;
}) {
  const update = (patch: Partial<ScheduleFields>) => onChange({ ...fields, ...patch });

  const toggleWeekday = (i: number) => {
    const next = [...fields.weekdays];
    next[i] = !next[i];
    update({ weekdays: next });
  };

  const showCalendar = fields.mode === "once";
  const showRepeatEvery = fields.mode === "interval";
  const showDayPicker =
    (fields.mode === "interval" && fields.intervalUnit === "weeks") ||
    (fields.mode === "cron" && fields.intervalUnit === "weeks");

  return (
    <div className="flex flex-col gap-4">
      {showCalendar && (
        <MiniCalendar
          value={fields.date}
          onSelect={(iso) => update({ date: iso })}
          showShortcuts={false}
        />
      )}

      {(fields.mode === "once" || fields.mode === "cron") && (
        <div className="flex flex-col gap-1.5">
          <span className="text-[11px] text-dim uppercase tracking-wider font-medium">Time</span>
          <input
            type="time"
            className="glass-input text-[13px] px-3 py-2 w-full rounded-lg"
            value={fields.time}
            onChange={(e) => update({ time: e.target.value })}
          />
        </div>
      )}

      <div className="border-t border-white/[0.08]" />

      <div className="flex flex-col gap-2">
        <span className="text-[11px] text-dim uppercase tracking-wider font-medium">Type</span>
        <div className="flex gap-4">
          {(
            [
              ["cron", "Scheduled"],
              ["once", "Once"],
              ["interval", "Set Interval"],
            ] as const
          ).map(([key, label]) => (
            <button
              key={key}
              type="button"
              onClick={() => {
                if (key === "once") update({ mode: "once" });
                else if (key === "interval")
                  update({ mode: "interval", intervalUnit: "hours", intervalValue: 1 });
                else update({ mode: "cron", intervalUnit: "days" });
              }}
              className="flex items-center gap-2 cursor-pointer group"
            >
              <span
                className={cn(
                  "w-4 h-4 rounded-full border-2 transition-colors flex items-center justify-center",
                  fields.mode === key
                    ? "border-brand"
                    : "border-white/20 group-hover:border-white/40",
                )}
              >
                {fields.mode === key && <span className="w-2 h-2 rounded-full bg-brand" />}
              </span>
              <span
                className={cn(
                  "text-[13px] transition-colors",
                  fields.mode === key ? "text-secondary font-medium" : "text-muted font-light",
                )}
              >
                {label}
              </span>
            </button>
          ))}
        </div>
      </div>

      {showRepeatEvery && (
        <div className="flex flex-col gap-2">
          <span className="text-[11px] text-dim uppercase tracking-wider font-medium">
            Repeat Every
          </span>
          <div className="flex items-center gap-2">
            <input
              type="number"
              min={1}
              className="glass-input w-16 text-[13px] text-center py-2 rounded-lg"
              value={fields.intervalValue}
              onChange={(e) => update({ intervalValue: Math.max(1, Number(e.target.value)) })}
            />
            <select
              className="glass-input text-[13px] px-3 py-2 flex-1 rounded-lg"
              value={fields.intervalUnit}
              onChange={(e) => update({ intervalUnit: e.target.value as IntervalUnit })}
            >
              <option value="minutes">Minutes</option>
              <option value="hours">Hours</option>
              <option value="days">Days</option>
              <option value="weeks">Weeks</option>
            </select>
          </div>
        </div>
      )}

      {fields.mode === "cron" && (
        <div className="flex flex-col gap-2">
          <span className="text-[11px] text-dim uppercase tracking-wider font-medium">
            Frequency
          </span>
          <div className="flex gap-2">
            {(["days", "weeks"] as const).map((u) => (
              <button
                key={u}
                type="button"
                onClick={() => update({ intervalUnit: u })}
                className={cn(
                  "px-4 py-2 text-[13px] rounded-lg font-medium transition-colors",
                  fields.intervalUnit === u
                    ? "bg-brand/20 text-brand border border-brand/30"
                    : "bg-white/[0.06] text-muted hover:text-secondary hover:bg-white/[0.1] border border-transparent",
                )}
              >
                {u === "days" ? "Every Day" : "Every Week"}
              </button>
            ))}
          </div>
        </div>
      )}

      {showDayPicker && (
        <div className="flex flex-col gap-2">
          <span className="text-[11px] text-dim uppercase tracking-wider font-medium">
            Execute On
          </span>
          <div className="flex gap-1.5">
            {WEEKDAYS.map((day, i) => (
              <button
                key={day}
                type="button"
                onClick={() => toggleWeekday(i)}
                className={cn(
                  "flex-1 py-2 rounded-lg text-[11px] font-semibold transition-colors",
                  fields.weekdays[i]
                    ? "bg-brand text-white"
                    : "bg-white/[0.06] text-dim hover:text-muted hover:bg-white/[0.1]",
                )}
              >
                {day}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

// ── Inline Schedule Cell ────────────────────────────────────────────────

function InlineScheduleCell({
  schedule,
  editable,
  onSave,
}: {
  schedule: CronSchedule;
  editable: boolean;
  onSave: (s: CronSchedule) => void;
}) {
  const [open, setOpen] = useState(false);
  const [fields, setFields] = useState<ScheduleFields>(() => parseSchedule(schedule));
  const triggerRef = useRef<HTMLButtonElement>(null);
  const [pos, setPos] = useState({ top: 0, left: 0 });

  const handleOpen = (e: React.MouseEvent) => {
    if (!editable) return;
    e.stopPropagation();
    setFields(parseSchedule(schedule));
    if (triggerRef.current) {
      const rect = triggerRef.current.getBoundingClientRect();
      const dropdownW = 310;
      let left = rect.right - dropdownW;
      if (left < 16) left = 16;
      if (left + dropdownW > window.innerWidth - 16) left = window.innerWidth - dropdownW - 16;
      const spaceBelow = window.innerHeight - rect.bottom - 16;
      const spaceAbove = rect.top - 16;
      const dropdownH = 520;
      let top: number;
      if (spaceBelow >= dropdownH) {
        top = rect.bottom + 4;
      } else if (spaceAbove >= dropdownH) {
        top = rect.top - dropdownH - 4;
      } else {
        top = Math.max(16, window.innerHeight - dropdownH - 16);
      }
      setPos({ top, left });
    }
    setOpen(true);
  };

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        onClick={handleOpen}
        className={cn(
          "text-[12px] font-light text-muted text-left truncate",
          editable && "cursor-text rounded px-1 -mx-1 transition-colors hover:bg-white/[0.06]",
        )}
      >
        {humanizeSchedule(schedule)}
      </button>
      {open &&
        createPortal(
          <>
            <div
              className="fixed inset-0 z-[9998]"
              onClick={() => setOpen(false)}
              onKeyDown={(e) => {
                if (e.key === "Escape") setOpen(false);
              }}
            />
            <div
              className="fixed z-[9999] glass-dropdown p-5 w-[310px] max-h-[calc(100vh-32px)] overflow-y-auto rounded-xl"
              style={{ top: pos.top, left: pos.left }}
              onClick={(e) => e.stopPropagation()}
              onKeyDown={(e) => e.stopPropagation()}
            >
              <SchedulePanel fields={fields} onChange={setFields} />
              <div className="flex gap-3 mt-5 pt-4 border-t border-white/[0.08]">
                <button
                  type="button"
                  onClick={() => {
                    onSave(fieldsToSchedule(fields));
                    setOpen(false);
                  }}
                  className="bg-brand hover:bg-brand-hover text-white px-5 py-2 rounded-lg text-[13px] font-medium transition-colors"
                >
                  Save
                </button>
                <button
                  type="button"
                  onClick={() => setOpen(false)}
                  className="glass-button px-5 py-2 text-[13px] font-medium rounded-lg"
                >
                  Cancel
                </button>
              </div>
            </div>
          </>,
          document.body,
        )}
    </>
  );
}

// ── Schedule Builder (for Create Form) ──────────────────────────────────

function JobScheduleBuilder({
  value,
  onChange,
}: {
  value: CronSchedule;
  onChange: (s: CronSchedule) => void;
}) {
  const [fields, setFields] = useState<ScheduleFields>(() => parseSchedule(value));

  const handleChange = useCallback(
    (f: ScheduleFields) => {
      setFields(f);
      onChange(fieldsToSchedule(f));
    },
    [onChange],
  );

  return <SchedulePanel fields={fields} onChange={handleChange} />;
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
  const {
    mutate,
    loading,
    error: createError,
  } = useMutation<CronJob, CronJobCreateParams>("cron_create", "params");

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

      {createError && <p className="text-destructive text-xs font-light">{createError.message}</p>}
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

// ── Constants ───────────────────────────────────────────────────────────

const ORIGIN_TABS: { key: OriginFilter; label: string }[] = [
  { key: "all", label: "All" },
  { key: "system", label: "System" },
  { key: "ai", label: "AI" },
  { key: "user", label: "User" },
  { key: "plugin", label: "Plugin" },
];

const getRowKey = (job: CronJob) => job.id;
const getRowClassName = (job: CronJob) => (!job.enabled ? "opacity-60" : "");

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
  const { mutate: updateJob } = useMutation<CronJob, CronJobUpdateParams>("cron_update", "params");

  const [originFilter, setOriginFilter] = useState<OriginFilter>("all");
  const [searchQ, setSearchQ] = useState("");
  const [debouncedQ, setDebouncedQ] = useState("");
  const debounceRef = useRef<ReturnType<typeof setTimeout>>();
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
      const result = await enableJob({ id, enabled });
      if (result) {
        invalidateQueries("cron_");
        refetch();
      }
    },
    [enableJob, refetch],
  );

  const handleRun = useCallback(
    async (id: string) => {
      const result = await runJob({ id });
      if (result !== undefined) {
        invalidateQueries("cron_");
        refetch();
      }
    },
    [runJob, refetch],
  );

  const handleDelete = useCallback(
    async (id: string) => {
      const result = await deleteJob({ id });
      if (result) {
        invalidateQueries("cron_");
        refetch();
      }
    },
    [deleteJob, refetch],
  );

  const handleUpdate = useCallback(
    async (id: string, field: Partial<CronJobUpdateParams>) => {
      const result = await updateJob({ id, ...field });
      if (result) {
        invalidateQueries("cron_");
        refetch();
      }
    },
    [updateJob, refetch],
  );

  const columns: DataTableColumn<CronJob>[] = useMemo(
    () => [
      {
        key: "name",
        header: "Name",
        width: "min-w-[180px]",
        renderCell: (job) => {
          const editable = job.origin !== "system";
          return (
            <InlineTextCell
              value={humanizeJobName(job.name)}
              editable={editable}
              onSave={(v) => handleUpdate(job.id, { name: v })}
              className={cn("text-[13px] font-light", job.enabled ? "text-secondary" : "text-dim")}
            />
          );
        },
      },
      {
        key: "origin",
        header: "Origin",
        width: "w-20",
        renderCell: (job) => <OriginBadge origin={job.origin} />,
      },
      {
        key: "type",
        header: "Type",
        width: "w-20",
        renderCell: (job) => <ScheduleTypeBadge schedule={job.schedule} />,
      },
      {
        key: "message",
        header: "Message",
        width: "min-w-[200px]",
        renderCell: (job) => {
          const editable = job.origin !== "system";
          return (
            <InlineTextCell
              value={job.payload.message}
              editable={editable}
              onSave={(v) => handleUpdate(job.id, { message: v })}
              className="text-[12px] font-light text-muted"
              placeholder="No message"
            />
          );
        },
      },
      {
        key: "schedule",
        header: "Schedule",
        width: "w-44",
        renderCell: (job) => {
          const editable = job.origin !== "system";
          return (
            <InlineScheduleCell
              schedule={job.schedule}
              editable={editable}
              onSave={(s) => handleUpdate(job.id, { schedule: s })}
            />
          );
        },
      },
      {
        key: "lastRun",
        header: "Last Run",
        width: "w-28",
        align: "right",
        renderCell: (job) => (
          <span className="text-[12px] font-light text-dim">
            {job.state.lastRunAtMs ? relativeTime(job.state.lastRunAtMs) : "—"}
            {job.state.lastStatus && job.state.lastStatus !== "ok" && (
              <span className="ml-1 text-destructive">!</span>
            )}
          </span>
        ),
      },
      {
        key: "nextRun",
        header: "Next Run",
        width: "w-24",
        align: "right",
        renderCell: (job) => (
          <span className="text-[12px] font-light text-dim">
            {job.enabled && job.state.nextRunAtMs ? relativeTime(job.state.nextRunAtMs) : "—"}
          </span>
        ),
      },
      {
        key: "actions",
        header: "",
        width: "w-20",
        align: "right",
        renderCell: (job) => (
          <div
            className="flex items-center justify-end gap-1"
            onClick={(e) => e.stopPropagation()}
            onKeyDown={(e) => e.stopPropagation()}
          >
            <button
              type="button"
              onClick={() => handleRun(job.id)}
              className="p-1 rounded text-dim hover:text-brand hover:bg-white/[0.06] transition-colors"
              title="Run now"
            >
              <Play size={12} />
            </button>
            {job.origin !== "system" && (
              <button
                type="button"
                onClick={() => handleDelete(job.id)}
                className="p-1 rounded text-dim hover:text-destructive hover:bg-red-500/10 transition-colors"
                title="Delete"
              >
                <Trash2 size={12} />
              </button>
            )}
          </div>
        ),
      },
    ],
    [handleUpdate, handleRun, handleDelete],
  );

  const renderRowPrefix = useCallback(
    (job: CronJob) => (
      <Toggle
        size="sm"
        checked={job.enabled}
        onChange={(checked) => handleEnable(job.id, checked)}
      />
    ),
    [handleEnable],
  );

  const emptyState = useMemo(
    () => (
      <div className="flex flex-col items-center justify-center py-16 text-dim">
        <Clock size={32} className="mb-3 opacity-30" />
        <p className="text-muted text-sm font-light">
          {debouncedQ || originFilter !== "all" ? "No matching automations" : "No automations yet"}
        </p>
        <p className="text-dim text-xs font-light mt-1">
          {debouncedQ || originFilter !== "all"
            ? "Try adjusting your filters"
            : "Create an automation to get started"}
        </p>
        {originFilter === "all" && !debouncedQ && (
          <button
            type="button"
            onClick={() => setShowCreate(true)}
            className="bg-brand hover:bg-brand-hover text-white mt-4 px-3 py-1.5 rounded-xl text-[12px] font-light flex items-center gap-2 transition-colors"
          >
            <Plus className="w-[14px] h-[14px]" strokeWidth={1.5} /> Create your first automation
          </button>
        )}
      </div>
    ),
    [debouncedQ, originFilter],
  );

  return (
    <div className="flex-1 flex flex-col gap-2 overflow-hidden">
      <div className="h-12 flex items-center px-2 shrink-0">
        <div className="flex-1 flex items-center gap-1.5">
          {ORIGIN_TABS.map((tab) => (
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

        <button
          type="button"
          onClick={() => setShowCreate(true)}
          className="bg-brand hover:bg-brand-hover text-white px-3 py-1.5 rounded-xl text-[12px] font-light flex items-center gap-2 transition-colors"
        >
          <Plus className="w-[14px] h-[14px]" strokeWidth={1.5} /> Add automation
        </button>
      </div>

      <div className="flex-1 flex flex-col overflow-hidden glass-card">
        {showCreate && (
          <AutomationCreateForm onClose={() => setShowCreate(false)} onCreated={refetch} />
        )}

        <div className="flex-1 overflow-y-auto overflow-x-auto">
          <DataTable<CronJob>
            columns={columns}
            data={filtered}
            rowKey={getRowKey}
            loading={loading}
            renderRowPrefix={renderRowPrefix}
            rowClassName={getRowClassName}
            emptyState={emptyState}
          />
        </div>
      </div>
    </div>
  );
}
