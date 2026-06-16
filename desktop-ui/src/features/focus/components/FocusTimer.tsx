import Coffee from "lucide-react/dist/esm/icons/coffee";
import Eye from "lucide-react/dist/esm/icons/eye";
import Pause from "lucide-react/dist/esm/icons/pause";
import Play from "lucide-react/dist/esm/icons/play";
import Settings from "lucide-react/dist/esm/icons/settings";
import Sparkles from "lucide-react/dist/esm/icons/sparkles";
import Square from "lucide-react/dist/esm/icons/square";
import X from "lucide-react/dist/esm/icons/x";
import { useRef, useState } from "react";
import { FOCUS_PRESETS, type useFocusTimer } from "../hooks/useFocusTimer";
import { formatElapsed, formatHumanDuration } from "@/lib/dates";
import { Checkbox } from "@/features/shared/components/Checkbox";
import "../focus.css";

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
      className="tc-icon-btn"
      onClick={timer.paused ? timer.resume : timer.pause}
      disabled={timer.isLoading}
      title={timer.paused ? "Resume" : "Pause"}
    >
      {timer.paused ? (
        <Play className="tc-icon-sm" strokeWidth={1.5} />
      ) : (
        <Pause className="tc-icon-sm" strokeWidth={1.5} />
      )}
    </button>
  );
}

function SettingsButton({ onClick }: { onClick: () => void }) {
  return (
    <button type="button" className="tc-icon-btn" onClick={onClick} title="Settings">
      <Settings className="tc-icon-sm" strokeWidth={1.5} />
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
    <div className="tc-distraction-row">
      {DISTRACTION_CATEGORIES.map((c) => (
        <button
          key={c.value}
          type="button"
          onClick={() => onLog(c.value)}
          className="tc-distraction-chip"
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
    <div className="tc-today-stats">
      Today: {stats.sessions} session{stats.sessions !== 1 ? "s" : ""} · {timeStr}
      {stats.avgQuality != null && <span> · {(stats.avgQuality * 100).toFixed(0)}% quality</span>}
    </div>
  );
}

export function FocusTimer({ timer, onOpenSettings }: { timer: Timer; onOpenSettings: () => void }) {
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
    ? "var(--tc-warning)"
    : isBreak
      ? "var(--tc-info)"
      : "var(--tc-brand)";

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
    <div className="tc-timer">
      {phase === "idle" && <TodayStats stats={timer.todayStats} />}

      <div className="tc-ring-wrap" style={{ width: RING_SIZE, height: RING_SIZE }}>
        <svg viewBox={`0 0 ${RING_SIZE} ${RING_SIZE}`} className="tc-ring-svg" aria-hidden="true">
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

        <div className="tc-ring-content">
          {isBreak || isBreakPending ? (
            <Coffee className="tc-mode-icon is-info" strokeWidth={1.5} />
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
              <div className="tc-time-edit-row">
                <input
                  ref={inputRef}
                  type="number"
                  value={editValue}
                  onChange={(e) => setEditValue(e.target.value)}
                  onBlur={handleEditSave}
                  min={1}
                  max={480}
                  className="tc-time-edit-input"
                />
                <span className="tc-time-edit-unit">min</span>
              </div>
            </form>
          ) : (
            <button
              type="button"
              onClick={handleEditStart}
              disabled={isActive || isBreakPending}
              className={`tc-time-button${paused ? " is-paused" : ""}`}
            >
              <span className="tc-time-display">{timeDisplay}</span>
            </button>
          )}

          <div className="tc-dots">
            {Array.from({ length: dotsCount }, (_, i) => `dot-${i}`).map((key, i) => (
              <div key={key} className={`tc-dot${i < filledDots ? " is-filled" : ""}`} />
            ))}
          </div>

          <span className="tc-phase-label">{phaseLabel}</span>

          {timer.actionTitle && phase === "working" && (
            <p className="tc-action-title">{timer.actionTitle}</p>
          )}
        </div>
      </div>

      {phase === "idle" && (
        <div className="tc-presets">
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

      {isWorking && !showWarning && <QuickDistractionLog onLog={timer.logDistraction} />}

      {showWarning && <WarningBanner timer={timer} isWorking={isWorking} />}

      {isBreakPending && <BreakPendingActions timer={timer} />}

      {isBreakPending && timer.coaching && (
        <CoachingDebrief message={timer.coaching.message} onDismiss={timer.dismissCoaching} />
      )}

      {!isBreak && !isBreakPending && !showWarning && (
        <div className="tc-dnd-row">
          <Checkbox
            id="tc-dnd"
            checked={settings.dndEnabled}
            onCheckedChange={(v) => timer.updateSettings({ dndEnabled: v })}
          />
          <label htmlFor="tc-dnd" className="tc-dnd-label">
            Do Not Disturb
          </label>
        </div>
      )}

      {dndHint && (
        <div className="tc-dnd-hint">
          <p className="tc-dnd-hint-text">{dndHint}</p>
          <button type="button" onClick={timer.dismissDndHint} className="tc-dnd-hint-close">
            <X className="tc-icon-xs" />
          </button>
        </div>
      )}

      {!isBreakPending && !showWarning && (
        <div className="tc-controls">
          {isBreak ? (
            <>
              <PauseResumeButton timer={timer} />
              <button
                type="button"
                onClick={timer.skipBreak}
                disabled={loading}
                className="tc-pill"
              >
                Skip
              </button>
              <button
                type="button"
                onClick={() => timer.stop()}
                disabled={loading}
                className="tc-pill is-danger"
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
                className="tc-pill"
              >
                Break
              </button>
              <button
                type="button"
                onClick={() => timer.stop()}
                disabled={loading}
                className="tc-pill is-danger"
              >
                Stop
              </button>
              <SettingsButton onClick={onOpenSettings} />
            </>
          ) : (
            <div className="tc-idle-controls">
              <button
                type="button"
                onClick={timer.start}
                disabled={loading}
                className="tc-pill is-wide"
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
    <div className="tc-coaching">
      <div className="tc-coaching-row">
        <Sparkles className="tc-coaching-icon" strokeWidth={1.5} />
        <p className="tc-coaching-text">{message}</p>
        <button type="button" onClick={onDismiss} className="tc-coaching-close">
          <X className="tc-icon-xs" />
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
    <div className="tc-warning">
      <p className="tc-warning-text">{isWorking ? "Focus ending soon" : "Break ending soon"}</p>
      <div className="tc-warning-actions">
        {extendOptions.map((opt) => (
          <button
            key={opt.secs}
            type="button"
            onClick={() => timer.extend(opt.secs)}
            disabled={timer.isLoading}
            className="tc-pill is-warning"
          >
            {opt.label}
          </button>
        ))}
        <button
          type="button"
          onClick={() => timer.stop()}
          disabled={timer.isLoading}
          className="tc-pill"
        >
          End now
        </button>
      </div>
    </div>
  );
}

function BreakPendingActions({ timer }: { timer: Timer }) {
  return (
    <div className="tc-break-pending">
      <p className="tc-break-pending-text">Break starting soon</p>
      <div className="tc-warning-actions">
        {[5, 10, 15].map((mins) => (
          <button
            key={mins}
            type="button"
            onClick={() => timer.extendWork(mins)}
            disabled={timer.isLoading}
            className="tc-pill"
          >
            +{mins}m work
          </button>
        ))}
      </div>
      <div className="tc-warning-actions">
        <button
          type="button"
          onClick={timer.startBreak}
          disabled={timer.isLoading}
          className="tc-pill"
        >
          <Coffee className="tc-icon-xs" strokeWidth={1.5} />
          Start Break
        </button>
        <button
          type="button"
          onClick={() => timer.stop()}
          disabled={timer.isLoading}
          className="tc-pill"
        >
          <Square className="tc-icon-xs" strokeWidth={1.5} />
          Stop
        </button>
      </div>
    </div>
  );
}
