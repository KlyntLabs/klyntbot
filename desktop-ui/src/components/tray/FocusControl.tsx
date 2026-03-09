import { ChevronRight, Eye, RotateCcw, Settings, X } from "lucide-react";
import { useRef, useState } from "react";
import type { FocusSettings, useFocusTimer } from "../../hooks/useFocusTimer";
import { formatElapsed } from "../../lib/dates";
import { Checkbox } from "../ui/Checkbox";

// ── SVG ring geometry ───────────────────────────────────────────────

const RING_SIZE = 170;
const STROKE = 5;
const CENTER = RING_SIZE / 2;
const RADIUS = CENTER - STROKE / 2 - 4;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

type Timer = ReturnType<typeof useFocusTimer>;

// ── Main export ─────────────────────────────────────────────────────

export function FocusControl({ timer }: { timer: Timer }) {
  const [view, setView] = useState<"timer" | "settings">("timer");

  if (view === "settings") {
    return (
      <FocusSettingsPanel
        settings={timer.settings}
        onUpdate={timer.updateSettings}
        onClose={() => setView("timer")}
      />
    );
  }

  return <TimerView timer={timer} onOpenSettings={() => setView("settings")} />;
}

// ── Timer view ──────────────────────────────────────────────────────

function TimerView({ timer, onOpenSettings }: { timer: Timer; onOpenSettings: () => void }) {
  const { active, remainingSecs, totalSecs, settings, completedSessions, completed, loading } =
    timer;

  const [editing, setEditing] = useState(false);
  const [editValue, setEditValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  // Progress: 0 → 1 as time elapses
  const progress =
    active && remainingSecs != null && totalSecs ? (totalSecs - remainingSecs) / totalSecs : 0;
  const dashOffset = CIRCUMFERENCE * (1 - progress);

  // Display time
  const timeDisplay =
    active && remainingSecs != null
      ? formatElapsed(remainingSecs)
      : `${String(settings.focusDuration).padStart(2, "0")}:00`;

  // Cycle state
  const cycleComplete = completedSessions > 0 && completedSessions >= settings.longBreakAfter;
  const dotsCount = settings.longBreakAfter;
  const filledDots = cycleComplete ? dotsCount : completedSessions;

  // Edit duration inline
  const handleEditStart = () => {
    if (active) return;
    setEditValue(String(settings.focusDuration));
    setEditing(true);
    requestAnimationFrame(() => inputRef.current?.select());
  };

  const handleEditSave = () => {
    const mins = Number.parseInt(editValue, 10);
    if (mins > 0 && mins <= 480) {
      timer.updateSettings({ focusDuration: mins });
    }
    setEditing(false);
  };

  return (
    <div className="flex flex-col items-center px-4 py-4">
      {/* Circular progress ring */}
      <div className="relative" style={{ width: RING_SIZE, height: RING_SIZE }}>
        <svg viewBox={`0 0 ${RING_SIZE} ${RING_SIZE}`} className="w-full h-full" aria-hidden="true">
          {/* Track */}
          <circle
            cx={CENTER}
            cy={CENTER}
            r={RADIUS}
            fill="none"
            stroke="rgba(255,255,255,0.07)"
            strokeWidth={STROKE}
          />
          {/* Progress arc */}
          {progress > 0 && (
            <circle
              cx={CENTER}
              cy={CENTER}
              r={RADIUS}
              fill="none"
              stroke="var(--brand)"
              strokeWidth={STROKE}
              strokeLinecap="round"
              strokeDasharray={CIRCUMFERENCE}
              strokeDashoffset={dashOffset}
              transform={`rotate(-90 ${CENTER} ${CENTER})`}
              className="transition-[stroke-dashoffset] duration-1000 ease-linear"
            />
          )}
        </svg>

        {/* Content inside ring */}
        <div className="absolute inset-0 flex flex-col items-center justify-center">
          <Eye
            className={`w-4 h-4 mb-2 transition-colors ${settings.dndEnabled ? "text-brand" : "text-dim"}`}
            strokeWidth={1.5}
          />

          {editing ? (
            <form
              onSubmit={(e) => {
                e.preventDefault();
                handleEditSave();
              }}
            >
              <div className="flex items-baseline justify-center gap-1">
                <input
                  ref={inputRef}
                  type="number"
                  value={editValue}
                  onChange={(e) => setEditValue(e.target.value)}
                  onBlur={handleEditSave}
                  min={1}
                  max={480}
                  className="w-16 text-center text-[36px] font-extralight tabular-nums text-primary bg-transparent outline-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                />
                <span className="text-[12px] text-dim font-light">min</span>
              </div>
            </form>
          ) : (
            <button
              type="button"
              onClick={handleEditStart}
              disabled={active}
              className="cursor-pointer disabled:cursor-default"
            >
              <span className="text-[36px] font-extralight tabular-nums text-primary leading-none">
                {timeDisplay}
              </span>
            </button>
          )}

          {/* Session dots */}
          <div className="flex gap-1.5 mt-2.5">
            {Array.from({ length: dotsCount }, (_, i) => (
              <div
                key={`dot-${i}`}
                className={`w-[6px] h-[6px] rounded-full transition-colors duration-300 ${
                  i < filledDots ? "bg-brand" : "bg-white/[0.15]"
                }`}
              />
            ))}
          </div>

          <span className="text-[10px] text-muted uppercase tracking-[0.2em] mt-1.5 font-light">
            {active ? "Focus" : completed ? (cycleComplete ? "Long Break" : "Break") : "Focus"}
          </span>
        </div>
      </div>

      {/* Completion message */}
      {completed && !active && (
        <p className="text-[11px] text-muted font-light mt-2 text-center animate-fade-in">
          {cycleComplete
            ? `Great cycle! Take a ${settings.longBreak}m break`
            : `Take a ${settings.shortBreak}m break`}
        </p>
      )}

      {/* DND toggle */}
      <button
        type="button"
        className="flex items-center gap-2 mt-3 cursor-pointer select-none"
        onClick={() => timer.updateSettings({ dndEnabled: !settings.dndEnabled })}
      >
        <Checkbox
          checked={settings.dndEnabled}
          onCheckedChange={(v) => timer.updateSettings({ dndEnabled: v })}
        />
        <span className="text-[11px] text-muted font-light">Do Not Disturb</span>
      </button>

      {/* Bottom controls */}
      <div className="flex items-center justify-between w-full mt-4">
        <button
          type="button"
          className="w-8 h-8 rounded-full border border-white/[0.1] flex items-center justify-center text-muted hover:text-secondary hover:border-white/[0.2] transition-colors"
          onClick={timer.resetSessions}
          title="Reset sessions"
        >
          <RotateCcw className="w-3.5 h-3.5" strokeWidth={1.5} />
        </button>

        <button
          type="button"
          onClick={active ? () => timer.stop() : () => timer.start()}
          disabled={loading}
          className="px-8 py-2 rounded-full bg-white/[0.08] text-[11px] uppercase tracking-[0.15em] text-primary font-light hover:bg-white/[0.12] transition-colors disabled:opacity-50"
        >
          {active ? "Stop" : "Start"}
        </button>

        <button
          type="button"
          className="w-8 h-8 rounded-full border border-white/[0.1] flex items-center justify-center text-muted hover:text-secondary hover:border-white/[0.2] transition-colors"
          onClick={onOpenSettings}
          title="Settings"
        >
          <Settings className="w-3.5 h-3.5" strokeWidth={1.5} />
        </button>
      </div>
    </div>
  );
}

// ── Settings panel ──────────────────────────────────────────────────

function FocusSettingsPanel({
  settings,
  onUpdate,
  onClose,
}: {
  settings: FocusSettings;
  onUpdate: (partial: Partial<FocusSettings>) => void;
  onClose: () => void;
}) {
  const [tab, setTab] = useState<"duration" | "notifications">("duration");

  return (
    <div className="px-4 py-4">
      {/* Header */}
      <div className="flex items-center mb-5">
        <div className="flex-1" />
        <span className="text-[15px] font-light text-primary">Settings</span>
        <div className="flex-1 flex justify-end">
          <button
            type="button"
            onClick={onClose}
            className="w-7 h-7 rounded-full border border-white/[0.1] flex items-center justify-center text-muted hover:text-secondary hover:border-white/[0.2] transition-colors"
          >
            <X className="w-3.5 h-3.5" strokeWidth={1.5} />
          </button>
        </div>
      </div>

      {/* Tab switcher */}
      <div className="flex p-0.5 rounded-full bg-white/[0.06] mb-5">
        <button
          type="button"
          onClick={() => setTab("duration")}
          className={`flex-1 py-1.5 rounded-full text-[10px] uppercase tracking-[0.12em] font-light transition-colors ${
            tab === "duration" ? "bg-white/[0.12] text-primary" : "text-muted hover:text-secondary"
          }`}
        >
          Duration
        </button>
        <button
          type="button"
          onClick={() => setTab("notifications")}
          className={`flex-1 py-1.5 rounded-full text-[10px] uppercase tracking-[0.12em] font-light transition-colors ${
            tab === "notifications"
              ? "bg-white/[0.12] text-primary"
              : "text-muted hover:text-secondary"
          }`}
        >
          Notifications
        </button>
      </div>

      {tab === "duration" ? (
        <div className="space-y-1">
          <SettingRow
            label="Focus Session"
            value={settings.focusDuration}
            unit="min"
            onChange={(v) => onUpdate({ focusDuration: v })}
          />
          <SettingRow
            label="Short break"
            value={settings.shortBreak}
            unit="min"
            onChange={(v) => onUpdate({ shortBreak: v })}
          />
          <SettingRow
            label="Long break"
            value={settings.longBreak}
            unit="min"
            onChange={(v) => onUpdate({ longBreak: v })}
          />
          <SettingRow
            label="Long break after"
            value={settings.longBreakAfter}
            unit="Sess."
            onChange={(v) => onUpdate({ longBreakAfter: v })}
            min={1}
            max={12}
          />
        </div>
      ) : (
        <div className="space-y-1">
          <div className="flex items-center justify-between py-2.5">
            <span className="text-[13px] text-secondary font-light">Sound</span>
            <Checkbox checked onCheckedChange={() => {}} />
          </div>
          <div className="flex items-center justify-between py-2.5">
            <span className="text-[13px] text-secondary font-light">Notification</span>
            <Checkbox checked onCheckedChange={() => {}} />
          </div>
          <div className="flex items-center justify-between py-2.5">
            <span className="text-[13px] text-secondary font-light">Do Not Disturb</span>
            <Checkbox
              checked={settings.dndEnabled}
              onCheckedChange={(v) => onUpdate({ dndEnabled: v })}
            />
          </div>
        </div>
      )}
    </div>
  );
}

// ── Setting row ─────────────────────────────────────────────────────

function SettingRow({
  label,
  value,
  unit,
  onChange,
  min = 1,
  max = 120,
}: {
  label: string;
  value: number;
  unit: string;
  onChange: (v: number) => void;
  min?: number;
  max?: number;
}) {
  const [editing, setEditing] = useState(false);
  const [editVal, setEditVal] = useState(String(value));
  const inputRef = useRef<HTMLInputElement>(null);

  const startEdit = () => {
    setEditVal(String(value));
    setEditing(true);
    requestAnimationFrame(() => inputRef.current?.select());
  };

  const save = () => {
    const n = Number.parseInt(editVal, 10);
    if (n >= min && n <= max) onChange(n);
    setEditing(false);
  };

  return (
    <div className="flex items-center justify-between py-2.5">
      <span className="text-[13px] text-secondary font-light">{label}</span>
      {editing ? (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            save();
          }}
          className="flex items-center gap-1"
        >
          <input
            ref={inputRef}
            type="number"
            value={editVal}
            onChange={(e) => setEditVal(e.target.value)}
            onBlur={save}
            min={min}
            max={max}
            className="w-10 text-right text-[13px] font-light text-primary bg-transparent outline-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
          />
          <span className="text-[11px] text-dim font-light">{unit}</span>
        </form>
      ) : (
        <button
          type="button"
          onClick={startEdit}
          className="flex items-center gap-1.5 text-muted hover:text-secondary transition-colors"
        >
          <span className="text-[13px] font-light tabular-nums">
            {String(value).padStart(2, "0")}
          </span>
          <span className="text-[11px] font-light">{unit}</span>
          <ChevronRight className="w-3.5 h-3.5" strokeWidth={1.5} />
        </button>
      )}
    </div>
  );
}
