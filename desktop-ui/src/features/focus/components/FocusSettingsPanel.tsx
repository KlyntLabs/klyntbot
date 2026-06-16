import "../focus.css";
import ChevronRight from "lucide-react/dist/esm/icons/chevron-right";
import X from "lucide-react/dist/esm/icons/x";
import { useRef, useState } from "react";
import { Checkbox } from "@/features/shared/components/Checkbox";
import type { FocusSettings } from "../types";

export function FocusSettingsPanel({
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
    <div className="tc-settings">
      <div className="tc-settings-header">
        <div className="tc-settings-spacer" />
        <span className="tc-settings-title">Settings</span>
        <div className="tc-settings-spacer tc-settings-spacer-right">
          <button type="button" onClick={onClose} className="tc-icon-btn">
            <X className="tc-icon-sm" strokeWidth={1.5} />
          </button>
        </div>
      </div>

      <div className="tc-tabs">
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
        <div className="tc-settings-list">
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
        <div className="tc-settings-list">
          <div className="tc-settings-row">
            <span className="tc-settings-label">Sound</span>
            <Checkbox
              checked={settings.soundEnabled}
              onCheckedChange={(v) => onUpdate({ soundEnabled: v })}
            />
          </div>
          <div className="tc-settings-row">
            <span className="tc-settings-label">Notification</span>
            <Checkbox
              checked={settings.notificationEnabled}
              onCheckedChange={(v) => onUpdate({ notificationEnabled: v })}
            />
          </div>
          <div className="tc-settings-row">
            <span className="tc-settings-label">Do Not Disturb</span>
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
    <div className="tc-settings-row">
      <span className="tc-settings-label">{label}</span>
      {editing ? (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            save();
          }}
          className="tc-settings-edit"
        >
          <input
            ref={inputRef}
            type="number"
            value={editVal}
            onChange={(e) => setEditVal(e.target.value)}
            onBlur={save}
            min={min}
            max={max}
            className="tc-settings-input"
          />
          <span className="tc-settings-unit">{unit}</span>
        </form>
      ) : (
        <button type="button" onClick={startEdit} className="tc-settings-value">
          <span className="tc-settings-num">{String(value).padStart(2, "0")}</span>
          <span className="tc-settings-unit">{unit}</span>
          <ChevronRight className="tc-icon-sm" strokeWidth={1.5} />
        </button>
      )}
    </div>
  );
}
