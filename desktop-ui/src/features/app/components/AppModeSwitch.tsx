import { type KeyboardEvent, type MouseEvent, useCallback, useRef } from "react";
import type { AppMode } from "../hooks/useAppMode";

const MODES: { id: AppMode; label: string }[] = [
  { id: "assistant", label: "Assistant" },
  { id: "code", label: "Code" },
];

export type AppModeSwitchProps = {
  mode: AppMode;
  onChange: (mode: AppMode) => void;
};

export function AppModeSwitch({ mode, onChange }: AppModeSwitchProps) {
  const buttonRefs = useRef<Record<AppMode, HTMLButtonElement | null>>({
    assistant: null,
    code: null,
  });

  const handleClick = useCallback(
    (next: AppMode) => (event: MouseEvent<HTMLButtonElement>) => {
      event.stopPropagation();
      if (next !== mode) {
        onChange(next);
      }
    },
    [mode, onChange],
  );

  const handleKeyDown = useCallback(
    (event: KeyboardEvent<HTMLDivElement>) => {
      if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") {
        return;
      }
      event.preventDefault();
      const idx = MODES.findIndex((m) => m.id === mode);
      const delta = event.key === "ArrowRight" ? 1 : -1;
      const nextIdx = (idx + delta + MODES.length) % MODES.length;
      const next = MODES[nextIdx].id;
      onChange(next);
      buttonRefs.current[next]?.focus();
    },
    [mode, onChange],
  );

  return (
    <div
      className="app-mode-switch"
      role="tablist"
      aria-label="App mode"
      data-tauri-drag-region="false"
      onKeyDown={handleKeyDown}
    >
      {MODES.map(({ id, label }) => {
        const isActive = id === mode;
        return (
          <button
            key={id}
            ref={(el) => {
              buttonRefs.current[id] = el;
            }}
            type="button"
            role="tab"
            aria-selected={isActive}
            tabIndex={isActive ? 0 : -1}
            data-tauri-drag-region="false"
            className={`app-mode-switch__btn${isActive ? " app-mode-switch__btn--active" : ""}`}
            onClick={handleClick(id)}
          >
            {label}
          </button>
        );
      })}
    </div>
  );
}
