import { useAutoTunerStatus } from "@features/autotuner";
import { MicroReviewPrompt } from "@features/coaching/components/MicroReviewPrompt";
import { FOCUS_PRESETS, type FocusSettings, type useFocusTimer } from "@shared/hooks/useFocusTimer";
import { useQuery } from "@shared/hooks/useQuery";
import { formatElapsed, formatHumanDuration } from "@shared/lib/dates";
import { isTauri } from "@shared/lib/utils";
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
import { useEffect, useRef, useState } from "react";

// ── SVG ring geometry ───────────────────────────────────────────────

const RING_SIZE = 170;
const STROKE = 5;
const CENTER = RING_SIZE / 2;
const RADIUS = CENTER - STROKE / 2 - 4;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

type Timer = ReturnType<typeof useFocusTimer>;

const ICON_BTN =
  "size-8 rounded-full border border-separator flex items-center justify-center text-fg-secondary hover:text-fg hover:border-separator transition-colors";

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
        <Play className="size-3.5" strokeWidth={1.5} />
      ) : (
        <Pause className="size-3.5" strokeWidth={1.5} />
      )}
    </button>
  );
}

function SettingsButton({ onClick }: { onClick: () => void }) {
  return (
    <button type="button" className={ICON_BTN} onClick={onClick} title="Settings">
      <Settings className="size-3.5" strokeWidth={1.5} />
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
          className="px-2 py-0.5 text-ui-xs rounded-md bg-control-hover/30
                     text-fg-secondary hover:text-fg transition-colors"
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
    <div className="text-center text-ui-xs text-fg-secondary">
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
    cyclePosition,
    longBreakAfter,
    loading,
    showWarning,
    dndHint,
  } = timer;

  const [editing, setEditing] = useState(false);
  const [editValue, setEditValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  const { data: autotunerStatus } = useAutoTunerStatus();
  const [learningBannerDismissed, setLearningBannerDismissed] = useState(false);

  // Micro-review prompt: show when idle and there are due flashcards
  const { data: dueCount } = useQuery<number>("flashcard_total_due", undefined, 0);
  const [reviewPromptDismissed, setReviewPromptDismissed] = useState(false);

  const handleReviewAccept = async () => {
    setReviewPromptDismissed(true);
    if (isTauri) {
      const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
      const { emit } = await import("@tauri-apps/api/event");
      const mainWindow = await WebviewWindow.getByLabel("main");
      if (mainWindow) {
        await emit("navigate", { path: "/learn?review=true" });
        await mainWindow.show();
        await mainWindow.setFocus();
      }
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().hide();
    }
  };

  // Reset the one-time banner each time a new focus session starts
  useEffect(() => {
    if (phase === "working") setLearningBannerDismissed(false);
  }, [phase]);

  const showLearningLine =
    phase === "working" &&
    (autotunerStatus?.champion?.days_active ?? 0) > 3 &&
    !learningBannerDismissed;

  // Auto-dismiss the learning banner after 8 seconds
  useEffect(() => {
    if (showLearningLine) {
      const timer = setTimeout(() => {
        setLearningBannerDismissed(true);
      }, 8000);
      return () => clearTimeout(timer);
    }
  }, [showLearningLine]);

  const isActive = phase === "working" || phase === "break";
  const isBreak = phase === "break";
  const isBreakPending = phase === "break_pending";
  const isWorking = phase === "working";

  // Progress: 0 → 1 as time elapses
  const progress =
    isActive && remainingSecs != null && totalSecs ? (totalSecs - remainingSecs) / totalSecs : 0;
  const dashOffset = CIRCUMFERENCE * (1 - progress);

  // Display time
  const timeDisplay =
    isActive && remainingSecs != null
      ? formatElapsed(remainingSecs)
      : isBreakPending
        ? formatElapsed(settings.shortBreak * 60)
        : formatElapsed(settings.focusDuration * 60);

  // Ring color: brand for focus, info-blue for break, warning pulse at 30s
  const ringColor = showWarning
    ? "var(--ds-status-warning)"
    : isBreak
      ? "var(--ds-status-info)"
      : "var(--ds-accent)";

  // Cycle state (from backend)
  const dotsCount = longBreakAfter;
  const filledDots = cyclePosition;

  // Phase label
  const phaseLabel = (() => {
    if (paused) return "Paused";
    switch (phase) {
      case "working":
        return "Focus";
      case "break":
        return "Break";
      case "break_pending":
        return "Break";
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
            <Coffee className="size-4 mb-2 text-status-info" strokeWidth={1.5} />
          ) : (
            <Eye
              className={`size-4 mb-2 transition-colors ${settings.dndEnabled ? "text-brand" : "text-fg-dim"}`}
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
                  className="w-16 text-center text-[36px] font-extralight tabular-nums text-fg bg-transparent outline-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                />
                <span className="text-ui-sm text-fg-dim font-light">min</span>
              </div>
            </form>
          ) : (
            <button
              type="button"
              onClick={handleEditStart}
              disabled={isActive || isBreakPending}
              className={`cursor-pointer disabled:cursor-default ${paused ? "animate-pulse" : ""}`}
            >
              <span className="text-[36px] font-extralight tabular-nums text-fg leading-none">
                {timeDisplay}
              </span>
            </button>
          )}

          {/* Session dots */}
          <div className="flex gap-1.5 mt-2.5">
            {Array.from({ length: dotsCount }, (_, i) => (
              <div
                // biome-ignore lint/suspicious/noArrayIndexKey: static session dots from Array.from
                key={`dot-${i}`}
                className={`w-[6px] h-[6px] rounded-full transition-colors duration-300 ${
                  i < filledDots ? "bg-brand" : "bg-control-hover"
                }`}
              />
            ))}
          </div>

          <span className="text-ui-xs text-fg-secondary uppercase tracking-[0.2em] mt-1.5 font-light">
            {phaseLabel}
          </span>

          {timer.actionTitle && phase === "working" && (
            <p className="text-ui-xs text-fg-secondary truncate max-w-[120px] mt-0.5">
              {timer.actionTitle}
            </p>
          )}

          {showLearningLine && (
            <p className="text-ui-xs text-fg-secondary/50 mt-1.5">Learning how you focus best...</p>
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
              className={`px-2.5 py-1 text-ui-xs rounded-full transition-colors
                ${
                  timer.activePreset === preset.label
                    ? "bg-brand/20 text-brand border border-brand/30"
                    : "bg-control-hover/30 text-fg-secondary hover:text-fg border border-transparent"
                }`}
            >
              {preset.label} {preset.focusDuration}/{preset.shortBreak}
            </button>
          ))}
        </div>
      )}

      {/* ── Micro-review prompt (idle + due cards) ────────────────── */}
      {phase === "idle" && !reviewPromptDismissed && (dueCount ?? 0) > 0 && (
        <div className="mt-3 px-1">
          <MicroReviewPrompt
            dueCount={dueCount ?? 0}
            onAccept={handleReviewAccept}
            onSkip={() => setReviewPromptDismissed(true)}
          />
        </div>
      )}

      {/* ── Distraction quick-log (working only, no warning) ────────── */}
      {isWorking && !showWarning && <QuickDistractionLog onLog={timer.logDistraction} />}

      {/* ── 30s warning banner ────────────────────────────────────── */}
      {showWarning && <WarningBanner timer={timer} isWorking={isWorking} />}

      {/* ── Break pending actions ─────────────────────────────────── */}
      {isBreakPending && <BreakPendingActions timer={timer} />}

      {/* ── Coaching debrief (break_pending only) ──────────────────── */}
      {isBreakPending && timer.coaching && (
        <CoachingDebrief message={timer.coaching.message} onDismiss={timer.dismissCoaching} />
      )}

      {/* ── DND toggle (only in idle/working without warning) ───────── */}
      {!isBreak && !isBreakPending && !showWarning && (
        /* biome-ignore lint/a11y/noLabelWithoutControl: Radix Checkbox renders its own input */
        <label className="flex items-center gap-2 mt-3 cursor-pointer select-none">
          <Checkbox
            checked={settings.dndEnabled}
            onCheckedChange={(v) => timer.updateSettings({ dndEnabled: v })}
          />
          <span className="text-ui-xs text-fg-secondary font-light">Do Not Disturb</span>
        </label>
      )}

      {/* ── DND unavailable hint ──────────────────────────────────── */}
      {dndHint && (
        <div className="flex items-center gap-2 mt-2 px-2">
          <p className="text-[10px] text-status-warning/70 font-light leading-tight flex-1">
            {dndHint}
          </p>
          <button
            type="button"
            onClick={timer.dismissDndHint}
            className="text-fg-secondary hover:text-fg shrink-0"
          >
            <X className="size-3" />
          </button>
        </div>
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
                className="px-4 py-2 rounded-full bg-control-hover text-ui-xs uppercase tracking-[0.15em] text-fg font-light hover:bg-control-hover transition-colors disabled:opacity-50"
              >
                Skip
              </button>
              <button
                type="button"
                onClick={() => timer.stop()}
                disabled={loading}
                className="px-4 py-2 rounded-full bg-control-hover text-ui-xs uppercase tracking-[0.15em] text-status-danger font-light hover:bg-status-danger/10 transition-colors disabled:opacity-50"
              >
                Stop
              </button>
              <SettingsButton onClick={onOpenSettings} />
            </>
          ) : isWorking ? (
            <>
              <PauseResumeButton timer={timer} />
              <button
                type="button"
                onClick={timer.takeBreak}
                disabled={loading}
                className="px-4 py-2 rounded-full bg-control-hover text-ui-xs uppercase tracking-[0.15em] text-fg font-light hover:bg-control-hover transition-colors disabled:opacity-50"
              >
                Break
              </button>
              <button
                type="button"
                onClick={() => timer.stop()}
                disabled={loading}
                className="px-4 py-2 rounded-full bg-control-hover text-ui-xs uppercase tracking-[0.15em] text-status-danger font-light hover:bg-status-danger/10 transition-colors disabled:opacity-50"
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
                className="px-8 py-2 rounded-full bg-control-hover text-ui-xs uppercase tracking-[0.15em] text-fg font-light hover:bg-control-hover transition-colors disabled:opacity-50"
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
        <Sparkles className="size-3.5 text-brand mt-0.5 shrink-0" />
        <p className="text-ui-sm text-fg leading-relaxed flex-1">{message}</p>
        <button
          type="button"
          onClick={onDismiss}
          className="text-fg-secondary hover:text-fg shrink-0"
        >
          <X className="size-3" />
        </button>
      </div>
    </div>
  );
}

// ── 30-second warning banner ────────────────────────────────────────

function WarningBanner({ timer, isWorking }: { timer: Timer; isWorking: boolean }) {
  const extendOptions = isWorking
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
      <p className="text-ui-xs text-status-warning font-light text-center">
        {isWorking ? "Focus ending soon" : "Break ending soon"}
      </p>
      <div className="flex gap-1.5">
        {extendOptions.map((opt) => (
          <button
            key={opt.secs}
            type="button"
            onClick={() => timer.extend(opt.secs)}
            disabled={timer.loading}
            className="px-2.5 py-1.5 text-ui-xs rounded-full bg-status-warning/20 text-status-warning hover:bg-status-warning/30 transition-colors disabled:opacity-50"
          >
            {opt.label}
          </button>
        ))}
        <button
          type="button"
          onClick={() => timer.stop()}
          disabled={timer.loading}
          className="px-2.5 py-1.5 text-ui-xs rounded-full bg-control-hover text-fg-secondary font-light hover:bg-control-hover transition-colors disabled:opacity-50"
        >
          End now
        </button>
      </div>
    </div>
  );
}

// ── Break pending (between focus end and break start) ────────────────

function BreakPendingActions({ timer }: { timer: Timer }) {
  return (
    <div className="flex flex-col items-center gap-2 mt-3 animate-fade-in">
      <p className="text-ui-xs text-fg-secondary font-light text-center">Break starting soon</p>

      <div className="flex gap-1.5">
        {[5, 10, 15].map((mins) => (
          <button
            key={mins}
            type="button"
            onClick={() => timer.extendWork(mins)}
            disabled={timer.loading}
            className="px-2 py-1.5 text-ui-xs rounded-full bg-control-hover text-fg-secondary font-light hover:bg-control-hover transition-colors disabled:opacity-50"
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
          className="flex items-center gap-1 px-3 py-1.5 rounded-full bg-control-hover text-ui-xs uppercase tracking-[0.1em] text-fg font-light hover:bg-control-hover transition-colors disabled:opacity-50"
        >
          <Coffee className="size-3" strokeWidth={1.5} />
          Start Break
        </button>
        <button
          type="button"
          onClick={() => timer.stop()}
          disabled={timer.loading}
          className="flex items-center gap-1 px-3 py-1.5 rounded-full bg-control-hover text-ui-xs uppercase tracking-[0.1em] text-fg-secondary font-light hover:bg-control-hover transition-colors disabled:opacity-50"
        >
          <Square className="size-3" strokeWidth={1.5} />
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
        <span className="text-[15px] font-light text-fg">Settings</span>
        <div className="flex-1 flex justify-end">
          <button
            type="button"
            onClick={onClose}
            className="size-7 rounded-full border border-separator flex items-center justify-center text-fg-secondary hover:text-fg hover:border-separator transition-colors"
          >
            <X className="size-3.5" strokeWidth={1.5} />
          </button>
        </div>
      </div>

      {/* Tab switcher */}
      <div className="flex p-0.5 rounded-full bg-control-hover mb-5">
        <button
          type="button"
          onClick={() => setTab("duration")}
          className={`flex-1 py-1.5 rounded-full text-ui-xs uppercase tracking-[0.12em] font-light transition-colors ${
            tab === "duration" ? "bg-control-hover text-fg" : "text-fg-secondary hover:text-fg"
          }`}
        >
          Duration
        </button>
        <button
          type="button"
          onClick={() => setTab("notifications")}
          className={`flex-1 py-1.5 rounded-full text-ui-xs uppercase tracking-[0.12em] font-light transition-colors ${
            tab === "notifications" ? "bg-control-hover text-fg" : "text-fg-secondary hover:text-fg"
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
            <span className="text-ui text-fg-secondary font-light">Sound</span>
            <Checkbox
              checked={settings.soundEnabled}
              onCheckedChange={(v) => onUpdate({ soundEnabled: !!v })}
            />
          </div>
          <div className="flex items-center justify-between py-2.5">
            <span className="text-ui text-fg-secondary font-light">Notification</span>
            <Checkbox
              checked={settings.notificationEnabled}
              onCheckedChange={(v) => onUpdate({ notificationEnabled: !!v })}
            />
          </div>
          <div className="flex items-center justify-between py-2.5">
            <span className="text-ui text-fg-secondary font-light">Do Not Disturb</span>
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
      <span className="text-ui text-fg-secondary font-light">{label}</span>
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
            className="w-10 text-right text-ui font-light text-fg bg-transparent outline-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
          />
          <span className="text-ui-xs text-fg-dim font-light">{unit}</span>
        </form>
      ) : (
        <button
          type="button"
          onClick={startEdit}
          className="flex items-center gap-1.5 text-fg-secondary hover:text-fg transition-colors"
        >
          <span className="text-ui font-light tabular-nums">{String(value).padStart(2, "0")}</span>
          <span className="text-ui-xs font-light">{unit}</span>
          <ChevronRight className="size-3.5" strokeWidth={1.5} />
        </button>
      )}
    </div>
  );
}
