import { FOCUS_PRESETS, type FocusSettings, type useFocusTimer } from "@shared/hooks/useFocusTimer";
import { formatElapsed, formatHumanDuration } from "@shared/lib/dates";
import { Checkbox } from "@shared/ui";
import {
  ChevronRight,
  Coffee,
  Eye,
  Pause,
  Play,
  Settings,
  Sparkles,
  Square,
  X,
} from "lucide-react";
import { useRef, useState } from "react";

// ── SVG ring geometry ───────────────────────────────────────────────

const RING_SIZE = 170;
const STROKE = 5;
const CENTER = RING_SIZE / 2;
const RADIUS = CENTER - STROKE / 2 - 4;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;
const WARNING_SECS = 30;

type Timer = ReturnType<typeof useFocusTimer>;

const ICON_BTN =
  "w-8 h-8 rounded-full border border-border flex items-center justify-center text-muted-foreground hover:text-foreground hover:border-border transition-colors";

function PauseResumeButton({ timer }: { timer: Timer }) {
  return (
    <button
      type="button"
      className={ICON_BTN}
      onClick={timer.paused ? timer.resume : timer.pause}
      disabled={timer.loading}
      title={timer.paused ? "Resume" : "Pause"}
    >
      {timer.paused ? (
        <Play className="w-3.5 h-3.5" strokeWidth={1.5} />
      ) : (
        <Pause className="w-3.5 h-3.5" strokeWidth={1.5} />
      )}
    </button>
  );
}

function SettingsButton({ onClick }: { onClick: () => void }) {
  return (
    <button type="button" className={ICON_BTN} onClick={onClick} title="Settings">
      <Settings className="w-3.5 h-3.5" strokeWidth={1.5} />
    </button>
  );
}

// ── Distraction quick-log ────────────────────────────────────────────

const DISTRACTION_CATEGORIES = [
  { label: "Social", value: "social_media" },
  { label: "Chat", value: "chat" },
  { label: "Email", value: "email" },
  { label: "Tired", value: "fatigue" },
  { label: "Meeting", value: "meeting" },
] as const;

function QuickDistractionLog({ onLog }: { onLog: (cat: string) => void }) {
  return (
    <div className="flex gap-1 justify-center flex-wrap px-2">
      {DISTRACTION_CATEGORIES.map((c) => (
        <button
          key={c.value}
          type="button"
          onClick={() => onLog(c.value)}
          className="px-2 py-0.5 text-[10px] rounded-md bg-muted/30
                     text-muted-foreground hover:text-foreground transition-colors"
        >
          {c.label}
        </button>
      ))}
    </div>
  );
}

// ── Today stats ─────────────────────────────────────────────────────

function TodayStats({
  stats,
}: {
  stats: { sessions: number; totalMins: number; avgQuality: number | null };
}) {
  if (stats.sessions === 0) return null;

  const timeStr = formatHumanDuration(stats.totalMins * 60);

  return (
    <div className="text-center text-[10px] text-muted-foreground">
      Today: {stats.sessions} session{stats.sessions !== 1 ? "s" : ""} · {timeStr}
      {stats.avgQuality != null && <span> · {(stats.avgQuality * 100).toFixed(0)}% quality</span>}
    </div>
  );
}

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
  const {
    phase,
    paused,
    remainingSecs,
    totalSecs,
    settings,
    completedSessions,
    completed,
    loading,
  } = timer;

  const [editing, setEditing] = useState(false);
  const [editValue, setEditValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  const isActive = phase === "focus" || phase === "break";
  const isBreak = phase === "break";
  const isBreakPending = phase === "break_pending";
  const isFocus = phase === "focus";
  const showWarning =
    isActive &&
    !paused &&
    remainingSecs != null &&
    remainingSecs <= WARNING_SECS &&
    remainingSecs > 0;

  // Progress: 0 → 1 as time elapses
  const progress =
    isActive && remainingSecs != null && totalSecs ? (totalSecs - remainingSecs) / totalSecs : 0;
  const dashOffset = CIRCUMFERENCE * (1 - progress);

  // Display time
  const timeDisplay =
    isActive && remainingSecs != null
      ? formatElapsed(remainingSecs)
      : isBreakPending
        ? formatElapsed((completed?.breakMins ?? settings.shortBreak) * 60)
        : formatElapsed(settings.focusDuration * 60);

  // Ring color: brand for focus, info-blue for break, warning pulse at 30s
  const ringColor = showWarning ? "var(--warning)" : isBreak ? "var(--info)" : "var(--brand)";

  // Cycle state
  const cycleComplete = completedSessions > 0 && completedSessions >= settings.longBreakAfter;
  const dotsCount = settings.longBreakAfter;
  const filledDots = cycleComplete ? dotsCount : completedSessions;

  // Phase label
  const phaseLabel = (() => {
    if (paused) return "Paused";
    switch (phase) {
      case "focus":
        return "Focus";
      case "break":
        return "Break";
      case "break_pending":
        return cycleComplete ? "Long Break" : "Break";
      default:
        return "Focus";
    }
  })();

  // Edit duration inline
  const handleEditStart = () => {
    if (isActive || isBreakPending) return;
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
      {/* ── Today stats (idle only) ──────────────────────────────── */}
      {phase === "idle" && <TodayStats stats={timer.todayStats} />}

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
              stroke={ringColor}
              strokeWidth={STROKE}
              strokeLinecap="round"
              strokeDasharray={CIRCUMFERENCE}
              strokeDashoffset={dashOffset}
              transform={`rotate(-90 ${CENTER} ${CENTER})`}
              className={`transition-[stroke-dashoffset] duration-1000 ease-linear ${
                paused ? "opacity-50" : showWarning ? "animate-pulse" : ""
              }`}
            />
          )}
        </svg>

        {/* Content inside ring */}
        <div className="absolute inset-0 flex flex-col items-center justify-center">
          {isBreak || isBreakPending ? (
            <Coffee className="w-4 h-4 mb-2 text-info" strokeWidth={1.5} />
          ) : (
            <Eye
              className={`w-4 h-4 mb-2 transition-colors ${settings.dndEnabled ? "text-brand" : "text-dim"}`}
              strokeWidth={1.5}
            />
          )}

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
                  className="w-16 text-center text-[36px] font-extralight tabular-nums text-foreground bg-transparent outline-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                />
                <span className="text-[12px] text-dim font-light">min</span>
              </div>
            </form>
          ) : (
            <button
              type="button"
              onClick={handleEditStart}
              disabled={isActive || isBreakPending}
              className={`cursor-pointer disabled:cursor-default ${paused ? "animate-pulse" : ""}`}
            >
              <span className="text-[36px] font-extralight tabular-nums text-foreground leading-none">
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
                  i < filledDots ? "bg-brand" : "bg-muted"
                }`}
              />
            ))}
          </div>

          <span className="text-[10px] text-muted-foreground uppercase tracking-[0.2em] mt-1.5 font-light">
            {phaseLabel}
          </span>

          {timer.actionTitle && phase === "focus" && (
            <p className="text-[10px] text-muted-foreground truncate max-w-[120px] mt-0.5">
              {timer.actionTitle}
            </p>
          )}
        </div>
      </div>

      {/* ── Presets (idle only) ─────────────────────────────────────── */}
      {phase === "idle" && (
        <div className="flex gap-1.5 justify-center mt-1">
          {FOCUS_PRESETS.map((preset) => (
            <button
              key={preset.label}
              type="button"
              onClick={() => timer.applyPreset(preset)}
              className={`px-2.5 py-1 text-[10px] rounded-full transition-colors
                ${
                  timer.activePreset === preset.label
                    ? "bg-brand/20 text-brand border border-brand/30"
                    : "bg-muted/30 text-muted-foreground hover:text-foreground border border-transparent"
                }`}
            >
              {preset.label} {preset.focusDuration}/{preset.shortBreak}
            </button>
          ))}
        </div>
      )}

      {/* ── Distraction quick-log (focus only, no warning) ────────── */}
      {isFocus && !showWarning && <QuickDistractionLog onLog={timer.logDistraction} />}

      {/* ── 30s warning banner ────────────────────────────────────── */}
      {showWarning && <WarningBanner timer={timer} isFocus={isFocus} />}

      {/* ── Break pending actions ─────────────────────────────────── */}
      {isBreakPending && <BreakPendingActions timer={timer} cycleComplete={cycleComplete} />}

      {/* ── Coaching debrief (break_pending only) ──────────────────── */}
      {isBreakPending && timer.coaching && (
        <CoachingDebrief message={timer.coaching.message} onDismiss={timer.dismissCoaching} />
      )}

      {/* ── DND toggle (only in idle/focus without warning) ───────── */}
      {!isBreak && !isBreakPending && !showWarning && (
        /* biome-ignore lint/a11y/noLabelWithoutControl: Radix Checkbox renders its own input */
        <label className="flex items-center gap-2 mt-3 cursor-pointer select-none">
          <Checkbox
            checked={settings.dndEnabled}
            onCheckedChange={(v) => timer.updateSettings({ dndEnabled: v })}
          />
          <span className="text-[11px] text-muted-foreground font-light">Do Not Disturb</span>
        </label>
      )}

      {/* ── Bottom controls ───────────────────────────────────────── */}
      {!isBreakPending && !showWarning && (
        <div className="flex items-center justify-between w-full mt-4">
          {isBreak ? (
            <>
              <PauseResumeButton timer={timer} />
              <button
                type="button"
                onClick={timer.skipBreak}
                disabled={loading}
                className="px-4 py-2 rounded-full bg-muted text-[11px] uppercase tracking-[0.15em] text-foreground font-light hover:bg-muted transition-colors disabled:opacity-50"
              >
                Skip
              </button>
              <button
                type="button"
                onClick={() => timer.stop()}
                disabled={loading}
                className="px-4 py-2 rounded-full bg-muted text-[11px] uppercase tracking-[0.15em] text-destructive font-light hover:bg-destructive/10 transition-colors disabled:opacity-50"
              >
                Stop
              </button>
              <SettingsButton onClick={onOpenSettings} />
            </>
          ) : isFocus ? (
            <>
              <PauseResumeButton timer={timer} />
              <button
                type="button"
                onClick={timer.takeBreak}
                disabled={loading}
                className="px-4 py-2 rounded-full bg-muted text-[11px] uppercase tracking-[0.15em] text-foreground font-light hover:bg-muted transition-colors disabled:opacity-50"
              >
                Break
              </button>
              <button
                type="button"
                onClick={() => timer.stop()}
                disabled={loading}
                className="px-4 py-2 rounded-full bg-muted text-[11px] uppercase tracking-[0.15em] text-destructive font-light hover:bg-destructive/10 transition-colors disabled:opacity-50"
              >
                Stop
              </button>
              <SettingsButton onClick={onOpenSettings} />
            </>
          ) : (
            <div className="flex items-center justify-center gap-3 w-full">
              <button
                type="button"
                onClick={timer.start}
                disabled={loading}
                className="px-8 py-2 rounded-full bg-muted text-[11px] uppercase tracking-[0.15em] text-foreground font-light hover:bg-muted transition-colors disabled:opacity-50"
              >
                Start
              </button>
              <SettingsButton onClick={onOpenSettings} />
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ── Coaching debrief ────────────────────────────────────────────────

function CoachingDebrief({ message, onDismiss }: { message: string; onDismiss: () => void }) {
  return (
    <div className="mx-2 p-2.5 rounded-lg bg-brand/10 border border-brand/20">
      <div className="flex items-start gap-2">
        <Sparkles className="w-3.5 h-3.5 text-brand mt-0.5 shrink-0" />
        <p className="text-xs text-foreground leading-relaxed flex-1">{message}</p>
        <button
          type="button"
          onClick={onDismiss}
          className="text-muted-foreground hover:text-foreground shrink-0"
        >
          <X className="w-3 h-3" />
        </button>
      </div>
    </div>
  );
}

// ── 30-second warning banner ────────────────────────────────────────

function WarningBanner({ timer, isFocus }: { timer: Timer; isFocus: boolean }) {
  const extendOptions = isFocus
    ? [
        { label: "+5m", secs: 300 },
        { label: "+10m", secs: 600 },
        { label: "+15m", secs: 900 },
      ]
    : [
        { label: "+30s", secs: 30 },
        { label: "+1m", secs: 60 },
        { label: "+2m", secs: 120 },
      ];

  return (
    <div className="flex flex-col items-center gap-2 mt-3 animate-fade-in">
      <p className="text-[11px] text-warning font-light text-center">
        {isFocus ? "Focus ending soon" : "Break ending soon"}
      </p>
      <div className="flex gap-1.5">
        {extendOptions.map((opt) => (
          <button
            key={opt.secs}
            type="button"
            onClick={() => timer.extend(opt.secs)}
            disabled={timer.loading}
            className="px-2.5 py-1.5 text-[10px] rounded-full bg-warning/20 text-warning hover:bg-warning/30 transition-colors disabled:opacity-50"
          >
            {opt.label}
          </button>
        ))}
        <button
          type="button"
          onClick={() => timer.stop()}
          disabled={timer.loading}
          className="px-2.5 py-1.5 text-[10px] rounded-full bg-accent text-muted-foreground font-light hover:bg-muted transition-colors disabled:opacity-50"
        >
          End now
        </button>
      </div>
    </div>
  );
}

// ── Break pending (between focus end and break start) ────────────────

function BreakPendingActions({ timer, cycleComplete }: { timer: Timer; cycleComplete: boolean }) {
  const breakMins = timer.completed?.breakMins ?? timer.settings.shortBreak;

  return (
    <div className="flex flex-col items-center gap-2 mt-3 animate-fade-in">
      <p className="text-[11px] text-muted-foreground font-light text-center">
        {cycleComplete
          ? `Great cycle! ${breakMins}m break starting soon`
          : `${breakMins}m break starting soon`}
      </p>

      <div className="flex gap-1.5">
        {[5, 10, 15].map((mins) => (
          <button
            key={mins}
            type="button"
            onClick={() => timer.extendWork(mins)}
            disabled={timer.loading}
            className="px-2 py-1.5 text-[10px] rounded-full bg-accent text-muted-foreground font-light hover:bg-muted transition-colors disabled:opacity-50"
          >
            +{mins}m work
          </button>
        ))}
      </div>
      <div className="flex gap-1.5">
        <button
          type="button"
          onClick={timer.startBreak}
          disabled={timer.loading}
          className="flex items-center gap-1 px-3 py-1.5 rounded-full bg-muted text-[10px] uppercase tracking-[0.1em] text-foreground font-light hover:bg-muted transition-colors disabled:opacity-50"
        >
          <Coffee className="w-3 h-3" strokeWidth={1.5} />
          Start Break
        </button>
        <button
          type="button"
          onClick={() => timer.stop()}
          disabled={timer.loading}
          className="flex items-center gap-1 px-3 py-1.5 rounded-full bg-accent text-[10px] uppercase tracking-[0.1em] text-muted-foreground font-light hover:bg-muted transition-colors disabled:opacity-50"
        >
          <Square className="w-3 h-3" strokeWidth={1.5} />
          Stop
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
        <span className="text-[15px] font-light text-foreground">Settings</span>
        <div className="flex-1 flex justify-end">
          <button
            type="button"
            onClick={onClose}
            className="w-7 h-7 rounded-full border border-border flex items-center justify-center text-muted-foreground hover:text-foreground hover:border-border transition-colors"
          >
            <X className="w-3.5 h-3.5" strokeWidth={1.5} />
          </button>
        </div>
      </div>

      {/* Tab switcher */}
      <div className="flex p-0.5 rounded-full bg-accent mb-5">
        <button
          type="button"
          onClick={() => setTab("duration")}
          className={`flex-1 py-1.5 rounded-full text-[10px] uppercase tracking-[0.12em] font-light transition-colors ${
            tab === "duration"
              ? "bg-muted text-foreground"
              : "text-muted-foreground hover:text-foreground"
          }`}
        >
          Duration
        </button>
        <button
          type="button"
          onClick={() => setTab("notifications")}
          className={`flex-1 py-1.5 rounded-full text-[10px] uppercase tracking-[0.12em] font-light transition-colors ${
            tab === "notifications"
              ? "bg-muted text-foreground"
              : "text-muted-foreground hover:text-foreground"
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
            <span className="text-[13px] text-muted-foreground font-light">Sound</span>
            <Checkbox
              checked={settings.soundEnabled}
              onCheckedChange={(v) => onUpdate({ soundEnabled: !!v })}
            />
          </div>
          <div className="flex items-center justify-between py-2.5">
            <span className="text-[13px] text-muted-foreground font-light">Notification</span>
            <Checkbox
              checked={settings.notificationEnabled}
              onCheckedChange={(v) => onUpdate({ notificationEnabled: !!v })}
            />
          </div>
          <div className="flex items-center justify-between py-2.5">
            <span className="text-[13px] text-muted-foreground font-light">Do Not Disturb</span>
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
      <span className="text-[13px] text-muted-foreground font-light">{label}</span>
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
            className="w-10 text-right text-[13px] font-light text-foreground bg-transparent outline-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
          />
          <span className="text-[11px] text-dim font-light">{unit}</span>
        </form>
      ) : (
        <button
          type="button"
          onClick={startEdit}
          className="flex items-center gap-1.5 text-muted-foreground hover:text-foreground transition-colors"
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
