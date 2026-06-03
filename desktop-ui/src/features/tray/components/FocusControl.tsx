import ChevronRight from "lucide-react/dist/esm/icons/chevron-right";
import Coffee from "lucide-react/dist/esm/icons/coffee";
import Eye from "lucide-react/dist/esm/icons/eye";
import Pause from "lucide-react/dist/esm/icons/pause";
import Play from "lucide-react/dist/esm/icons/play";
import Settings from "lucide-react/dist/esm/icons/settings";
import Sparkles from "lucide-react/dist/esm/icons/sparkles";
import Square from "lucide-react/dist/esm/icons/square";
import X from "lucide-react/dist/esm/icons/x";
import { useEffect, useRef, useState } from "react";
import { cn } from "@/utils/cn";
import { qk, useTauriQuery } from "@/lib/query";
import { emit, getCurrentWindow, getWindowByLabel, isTauri } from "@/utils/tauri-bridge";
import { FOCUS_PRESETS, type useFocusTimer } from "../hooks/useFocusTimer";
import { formatElapsed, formatHumanDuration } from "../lib/dates";
import type { FocusSettings } from "../types";
import { Checkbox } from "./Checkbox";
import { MicroReviewPrompt, useAutoTunerStatus } from "./stubs";

const RING_SIZE = 170;
const STROKE = 5;
const CENTER = RING_SIZE / 2;
const RADIUS = CENTER - STROKE / 2 - 4;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

type Timer = ReturnType<typeof useFocusTimer>;

function PauseResumeButton({ timer }: { timer: Timer }) {
  return (
    <button
      type="button"
      className="w-8 h-8 rounded-full bg-transparent flex items-center justify-center text-text-muted cursor-pointer"
      onClick={timer.paused ? timer.resume : timer.pause}
      disabled={timer.isLoading}
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
    <button type="button" className="w-8 h-8 rounded-full bg-transparent flex items-center justify-center text-text-muted cursor-pointer" onClick={onClick} title="Settings">
      <Settings className="w-3.5 h-3.5" strokeWidth={1.5} />
    </button>
  );
}

const DISTRACTION_CATEGORIES = [
  { label: "Social", value: "social_media" },
  { label: "Chat", value: "chat" },
  { label: "Email", value: "email" },
  { label: "Tired", value: "fatigue" },
  { label: "Meeting", value: "meeting" },
] as const;

function QuickDistractionLog({ onLog }: { onLog: (cat: string) => void }) {
  return (
    <div className="flex gap-1 justify-center flex-wrap px-2 mt-3">
      {DISTRACTION_CATEGORIES.map((c) => (
        <button
          key={c.value}
          type="button"
          onClick={() => onLog(c.value)}
          className="py-0.5 px-2 rounded-md bg-surface-control border-none text-text-muted text-ui-3xs cursor-pointer"
        >
          {c.label}
        </button>
      ))}
    </div>
  );
}

function TodayStats({
  stats,
}: {
  stats: { sessions: number; totalMins: number; avgQuality: number | null };
}) {
  if (stats.sessions === 0) return null;
  const timeStr = formatHumanDuration(stats.totalMins * 60);
  return (
    <div className="text-ui-3xs text-center text-text-muted mb-1">
      Today: {stats.sessions} session{stats.sessions !== 1 ? "s" : ""} · {timeStr}
      {stats.avgQuality != null && <span> · {(stats.avgQuality * 100).toFixed(0)}% quality</span>}
    </div>
  );
}

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

function TimerView({ timer, onOpenSettings }: { timer: Timer; onOpenSettings: () => void }) {
  const {
    phase,
    paused,
    remainingSecs,
    totalSecs,
    settings,
    cyclePosition,
    longBreakAfter,
    isLoading: loading,
    showWarning,
    dndHint,
  } = timer;

  const [editing, setEditing] = useState(false);
  const [editValue, setEditValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  const { data: autotunerStatus } = useAutoTunerStatus();
  const [learningBannerDismissed, setLearningBannerDismissed] = useState(false);

  const { data: dueCount } = useTauriQuery<number>({
    queryKey: qk.flashcards.dueCount(),
    command: "flashcard_total_due",
    fallback: 0,
  });
  const [reviewPromptDismissed, setReviewPromptDismissed] = useState(false);

  const handleReviewAccept = async () => {
    setReviewPromptDismissed(true);
    if (!isTauri()) return;
    try {
      await emit("navigate", { path: "/learn?review=true" });
      const main = getWindowByLabel("main");
      await main.show();
      await main.setFocus();
      await getCurrentWindow().hide();
    } catch {
      // silent
    }
  };

  useEffect(() => {
    if (phase === "working") setLearningBannerDismissed(false);
  }, [phase]);

  const showLearningLine =
    phase === "working" &&
    (autotunerStatus?.champion?.days_active ?? 0) > 3 &&
    !learningBannerDismissed;

  useEffect(() => {
    if (showLearningLine) {
      const t = setTimeout(() => setLearningBannerDismissed(true), 8000);
      return () => clearTimeout(t);
    }
  }, [showLearningLine]);

  const isActive = phase === "working" || phase === "break";
  const isBreak = phase === "break";
  const isBreakPending = phase === "break_pending";
  const isWorking = phase === "working";

  const progress =
    isActive && remainingSecs != null && totalSecs ? (totalSecs - remainingSecs) / totalSecs : 0;
  const dashOffset = CIRCUMFERENCE * (1 - progress);

  const timeDisplay =
    isActive && remainingSecs != null
      ? formatElapsed(remainingSecs)
      : isBreakPending
        ? formatElapsed(settings.shortBreak * 60)
        : formatElapsed(settings.focusDuration * 60);

  const ringColor = showWarning
    ? "var(--tray-warning)"
    : isBreak
      ? "var(--tray-info)"
      : "var(--tray-brand)";

  const dotsCount = longBreakAfter;
  const filledDots = cyclePosition;

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
    <div className="flex flex-col items-center p-4">
      {phase === "idle" && <TodayStats stats={timer.todayStats} />}

      <div className="relative" style={{ width: RING_SIZE, height: RING_SIZE }}>
        <svg viewBox={`0 0 ${RING_SIZE} ${RING_SIZE}`} className="w-full h-full" aria-hidden="true">
          <circle
            cx={CENTER}
            cy={CENTER}
            r={RADIUS}
            fill="none"
            stroke="rgba(255,255,255,0.07)"
            strokeWidth={STROKE}
          />
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
              className={`tc-ring-progress${paused ? " is-paused" : ""}${
                showWarning ? " is-warning" : ""
              }`}
            />
          )}
        </svg>

        <div className="absolute inset-0 flex flex-col items-center justify-center">
          {isBreak || isBreakPending ? (
            <Coffee className="w-4 h-4 mb-2 text-[var(--tray-info)]" strokeWidth={1.5} />
          ) : (
            <Eye
              className={`tc-mode-icon${settings.dndEnabled ? " is-brand" : " is-dim"}`}
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
                  className="w-16 text-center text-[36px] font-extralight text-text-primary bg-transparent border-none outline-none tabular-nums"
                />
                <span className="text-ui-2xs text-text-dim font-light">min</span>
              </div>
            </form>
          ) : (
            <button
              type="button"
              onClick={handleEditStart}
              disabled={isActive || isBreakPending}
              className={cn("bg-transparent border-none text-inherit cursor-pointer p-0", paused && "animate-[tray-pulse_1.2s_ease-in-out_infinite]")}
            >
              <span className="text-[36px] font-extralight text-text-primary tabular-nums leading-none">{timeDisplay}</span>
            </button>
          )}

          <div className="flex gap-1.5 mt-2.5">
            {Array.from({ length: dotsCount }, (_, i) => `dot-${i}`).map((key, i) => (
              <div key={key} className={`tc-dot${i < filledDots ? " is-filled" : ""}`} />
            ))}
          </div>

          <span className="text-ui-3xs text-text-muted uppercase tracking-[0.2em] mt-1.5 font-light">{phaseLabel}</span>

          {timer.actionTitle && phase === "working" && (
            <p className="text-ui-3xs text-text-muted max-w-[120px] mt-0.5 whitespace-nowrap overflow-hidden text-ellipsis">{timer.actionTitle}</p>
          )}

          {showLearningLine && <p className="text-ui-2xs mt-1.5">Learning how you focus best...</p>}
        </div>
      </div>

      {phase === "idle" && (
        <div className="flex gap-1.5 justify-center mt-1">
          {FOCUS_PRESETS.map((preset) => (
            <button
              key={preset.label}
              type="button"
              onClick={() => timer.applyPreset(preset)}
              className={`tc-preset${timer.activePreset === preset.label ? " is-active" : ""}`}
            >
              {preset.label} {preset.focusDuration}/{preset.shortBreak}
            </button>
          ))}
        </div>
      )}

      {phase === "idle" && !reviewPromptDismissed && (dueCount ?? 0) > 0 && (
        <div className="mt-3 px-1 w-full">
          <MicroReviewPrompt
            dueCount={dueCount ?? 0}
            onAccept={handleReviewAccept}
            onSkip={() => setReviewPromptDismissed(true)}
          />
        </div>
      )}

      {isWorking && !showWarning && <QuickDistractionLog onLog={timer.logDistraction} />}

      {showWarning && <WarningBanner timer={timer} isWorking={isWorking} />}

      {isBreakPending && <BreakPendingActions timer={timer} />}

      {isBreakPending && timer.coaching && (
        <CoachingDebrief message={timer.coaching.message} onDismiss={timer.dismissCoaching} />
      )}

      {!isBreak && !isBreakPending && !showWarning && (
        <div className="flex items-center gap-2 mt-3 cursor-pointer select-none">
          <Checkbox
            id="tc-dnd"
            checked={settings.dndEnabled}
            onCheckedChange={(v) => timer.updateSettings({ dndEnabled: v })}
          />
          <label htmlFor="tc-dnd" className="text-ui-2xs text-text-muted font-light">
            Do Not Disturb
          </label>
        </div>
      )}

      {dndHint && (
        <div className="flex items-center gap-2 mt-2 px-2">
          <p className="text-ui-3xs font-light leading-[1.3] flex-1 m-0">{dndHint}</p>
          <button type="button" onClick={timer.dismissDndHint} className="bg-transparent border-none text-text-muted cursor-pointer shrink-0">
            <X className="w-3 h-3" />
          </button>
        </div>
      )}

      {!isBreakPending && !showWarning && (
        <div className="flex items-center justify-between w-full mt-4">
          {isBreak ? (
            <>
              <PauseResumeButton timer={timer} />
              <button
                type="button"
                onClick={timer.skipBreak}
                disabled={loading}
                className="py-1.5 px-4 rounded-full bg-surface-control border-none text-text-primary text-ui-2xs font-light uppercase tracking-[0.12em] cursor-pointer inline-flex items-center gap-1"
              >
                Skip
              </button>
              <button
                type="button"
                onClick={() => timer.stop()}
                disabled={loading}
                className="py-1.5 px-4 rounded-full bg-surface-control border-none text-text-primary text-ui-2xs font-light uppercase tracking-[0.12em] cursor-pointer inline-flex items-center gap-1 text-[var(--tray-destructive)]"
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
                className="py-1.5 px-4 rounded-full bg-surface-control border-none text-text-primary text-ui-2xs font-light uppercase tracking-[0.12em] cursor-pointer inline-flex items-center gap-1"
              >
                Break
              </button>
              <button
                type="button"
                onClick={() => timer.stop()}
                disabled={loading}
                className="py-1.5 px-4 rounded-full bg-surface-control border-none text-text-primary text-ui-2xs font-light uppercase tracking-[0.12em] cursor-pointer inline-flex items-center gap-1 text-[var(--tray-destructive)]"
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
                className="py-2 px-8 rounded-full bg-surface-control border-none text-text-primary text-ui-2xs font-light uppercase tracking-[0.12em] cursor-pointer inline-flex items-center gap-1"
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

function CoachingDebrief({ message, onDismiss }: { message: string; onDismiss: () => void }) {
  return (
    <div className="m-2 p-2.5 rounded-lg">
      <div className="flex items-start gap-2">
        <Sparkles className="w-3.5 h-3.5 mt-0.5 shrink-0" strokeWidth={1.5} />
        <p className="flex-1 text-ui-xs text-text-primary leading-normal m-0">{message}</p>
        <button type="button" onClick={onDismiss} className="bg-transparent border-none text-text-muted cursor-pointer shrink-0">
          <X className="w-3 h-3" />
        </button>
      </div>
    </div>
  );
}

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
    <div className="flex flex-col items-center gap-2 mt-3">
      <p className="text-ui-2xs font-light text-center">{isWorking ? "Focus ending soon" : "Break ending soon"}</p>
      <div className="flex gap-1.5 flex-wrap justify-center">
        {extendOptions.map((opt) => (
          <button
            key={opt.secs}
            type="button"
            onClick={() => timer.extend(opt.secs)}
            disabled={timer.isLoading}
            className="py-1.5 px-4 rounded-full bg-surface-control border-none text-text-primary text-ui-2xs font-light uppercase tracking-[0.12em] cursor-pointer inline-flex items-center gap-1 bg-[var(--tray-warning-bg)] text-[var(--tray-warning)]"
          >
            {opt.label}
          </button>
        ))}
        <button
          type="button"
          onClick={() => timer.stop()}
          disabled={timer.isLoading}
          className="py-1.5 px-4 rounded-full bg-surface-control border-none text-text-primary text-ui-2xs font-light uppercase tracking-[0.12em] cursor-pointer inline-flex items-center gap-1"
        >
          End now
        </button>
      </div>
    </div>
  );
}

function BreakPendingActions({ timer }: { timer: Timer }) {
  return (
    <div className="flex flex-col items-center gap-2 mt-3">
      <p className="text-ui-2xs text-text-muted font-light">Break starting soon</p>
      <div className="flex gap-1.5 flex-wrap justify-center">
        {[5, 10, 15].map((mins) => (
          <button
            key={mins}
            type="button"
            onClick={() => timer.extendWork(mins)}
            disabled={timer.isLoading}
            className="py-1.5 px-4 rounded-full bg-surface-control border-none text-text-primary text-ui-2xs font-light uppercase tracking-[0.12em] cursor-pointer inline-flex items-center gap-1"
          >
            +{mins}m work
          </button>
        ))}
      </div>
      <div className="flex gap-1.5 flex-wrap justify-center">
        <button
          type="button"
          onClick={timer.startBreak}
          disabled={timer.isLoading}
          className="py-1.5 px-4 rounded-full bg-surface-control border-none text-text-primary text-ui-2xs font-light uppercase tracking-[0.12em] cursor-pointer inline-flex items-center gap-1"
        >
          <Coffee className="w-3 h-3" strokeWidth={1.5} />
          Start Break
        </button>
        <button
          type="button"
          onClick={() => timer.stop()}
          disabled={timer.isLoading}
          className="py-1.5 px-4 rounded-full bg-surface-control border-none text-text-primary text-ui-2xs font-light uppercase tracking-[0.12em] cursor-pointer inline-flex items-center gap-1"
        >
          <Square className="w-3 h-3" strokeWidth={1.5} />
          Stop
        </button>
      </div>
    </div>
  );
}

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
    <div className="p-4">
      <div className="flex items-center mb-5">
        <div className="flex-1" />
        <span className="text-ui-lg text-text-primary font-light">Settings</span>
        <div className="tc-settings-spacer tc-settings-spacer-right">
          <button type="button" onClick={onClose} className="w-8 h-8 rounded-full bg-transparent flex items-center justify-center text-text-muted cursor-pointer">
            <X className="w-3.5 h-3.5" strokeWidth={1.5} />
          </button>
        </div>
      </div>

      <div className="flex p-0.5 rounded-full bg-surface-control mb-5">
        <button
          type="button"
          onClick={() => setTab("duration")}
          className={`tc-tab${tab === "duration" ? " is-active" : ""}`}
        >
          Duration
        </button>
        <button
          type="button"
          onClick={() => setTab("notifications")}
          className={`tc-tab${tab === "notifications" ? " is-active" : ""}`}
        >
          Notifications
        </button>
      </div>

      {tab === "duration" ? (
        <div className="flex flex-col gap-1">
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
        <div className="flex flex-col gap-1">
          <div className="flex items-center justify-between py-2.5">
            <span className="text-ui-sm text-text-muted font-light">Sound</span>
            <Checkbox
              checked={settings.soundEnabled}
              onCheckedChange={(v) => onUpdate({ soundEnabled: !!v })}
            />
          </div>
          <div className="flex items-center justify-between py-2.5">
            <span className="text-ui-sm text-text-muted font-light">Notification</span>
            <Checkbox
              checked={settings.notificationEnabled}
              onCheckedChange={(v) => onUpdate({ notificationEnabled: !!v })}
            />
          </div>
          <div className="flex items-center justify-between py-2.5">
            <span className="text-ui-sm text-text-muted font-light">Do Not Disturb</span>
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
      <span className="text-ui-sm text-text-muted font-light">{label}</span>
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
            className="w-10 text-right text-ui-sm font-light text-text-primary bg-transparent border-none outline-none tabular-nums"
          />
          <span className="text-ui-2xs text-text-dim font-light">{unit}</span>
        </form>
      ) : (
        <button type="button" onClick={startEdit} className="flex items-center gap-1.5 bg-transparent border-none text-text-muted cursor-pointer">
          <span className="text-ui-sm font-light tabular-nums">{String(value).padStart(2, "0")}</span>
          <span className="text-ui-2xs text-text-dim font-light">{unit}</span>
          <ChevronRight className="w-3.5 h-3.5" strokeWidth={1.5} />
        </button>
      )}
    </div>
  );
}
