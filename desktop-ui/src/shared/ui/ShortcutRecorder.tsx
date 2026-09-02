import { RotateCcw } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

/** Maps browser event.code to Tauri-compatible key string. */
const SPECIAL_KEY_MAP: Record<string, string> = {
  Space: "space",
  Enter: "enter",
  Escape: "escape",
  Backspace: "backspace",
  Tab: "tab",
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
  Delete: "delete",
  Home: "home",
  End: "end",
  PageUp: "pageup",
  PageDown: "pagedown",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Backquote: "`",
};

function codeToTauriKey(code: string): string | null {
  // Filter out bare modifier keys — these are captured via boolean flags
  if (/^(Meta|Shift|Control|Alt)(Left|Right)$/.test(code)) return null;
  if (code.startsWith("Key")) return code.slice(3).toLowerCase(); // KeyC → c
  if (code.startsWith("Digit")) return code.slice(5); // Digit0 → 0
  return SPECIAL_KEY_MAP[code] ?? code.toLowerCase();
}

/** Build Tauri shortcut string from modifier flags + key. */
function buildShortcutString(e: KeyboardEvent): string | null {
  const key = codeToTauriKey(e.code);
  if (!key) return null; // Pure modifier press

  const parts: string[] = [];
  if (e.ctrlKey) parts.push("ctrl");
  if (e.altKey) parts.push("alt");
  if (e.metaKey) parts.push("super");
  if (e.shiftKey) parts.push("shift");

  // Require at least one modifier
  if (parts.length === 0) return null;

  parts.push(key);
  return parts.join("+");
}

/** Maps Tauri shortcut string to macOS display symbols. */
function displayShortcut(shortcut: string): string {
  return shortcut
    .split("+")
    .map((part) => {
      switch (part.toLowerCase()) {
        case "super":
          return "⌘";
        case "alt":
          return "⌥";
        case "shift":
          return "⇧";
        case "ctrl":
          return "⌃";
        case "space":
          return "Space";
        default:
          return part.toUpperCase();
      }
    })
    .join("");
}

export interface ShortcutRecorderProps {
  value: string;
  defaultValue: string;
  onChange: (value: string) => void;
  error?: string;
}

export function ShortcutRecorder({ value, defaultValue, onChange, error }: ShortcutRecorderProps) {
  const [recording, setRecording] = useState(false);
  const btnRef = useRef<HTMLButtonElement>(null);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      // Escape cancels recording
      if (e.code === "Escape") {
        setRecording(false);
        return;
      }

      const shortcut = buildShortcutString(e);
      if (shortcut) {
        onChange(shortcut);
        setRecording(false);
      }
    },
    [onChange],
  );

  // Attach keydown + blur listeners while recording
  useEffect(() => {
    if (!recording) return;

    const el = btnRef.current;
    const handleBlur = () => setRecording(false);

    window.addEventListener("keydown", handleKeyDown, true);
    el?.addEventListener("blur", handleBlur);

    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
      el?.removeEventListener("blur", handleBlur);
    };
  }, [recording, handleKeyDown]);

  const isDefault = value === defaultValue;

  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center gap-2">
        <button
          ref={btnRef}
          type="button"
          onClick={() => setRecording(true)}
          className={`flex-1 px-3 py-1.5 text-ui text-left rounded-control border transition-all ${
            recording
              ? "border-brand bg-control-hover animate-pulse text-brand"
              : error
                ? "border-status-danger/50 bg-control-hover text-fg"
                : "border-separator bg-control-hover text-fg hover:border-brand/30"
          }`}
        >
          {recording ? "Press shortcut..." : displayShortcut(value)}
        </button>
        {!isDefault && (
          <button
            type="button"
            onClick={() => onChange(defaultValue)}
            title="Reset to default"
            className="p-1.5 rounded-control text-fg-secondary hover:text-fg hover:bg-control-hover transition-colors"
          >
            <RotateCcw className="size-3.5" />
          </button>
        )}
      </div>
      {error && <p className="text-ui-xs text-status-danger">{error}</p>}
    </div>
  );
}
