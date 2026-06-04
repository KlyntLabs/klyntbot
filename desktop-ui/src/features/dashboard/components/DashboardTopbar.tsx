import Calendar from "lucide-react/dist/esm/icons/calendar";
import ChevronLeft from "lucide-react/dist/esm/icons/chevron-left";
import ChevronRight from "lucide-react/dist/esm/icons/chevron-right";
import Layers from "lucide-react/dist/esm/icons/layers";
import PanelRight from "lucide-react/dist/esm/icons/panel-right";
import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { formatFullDate, formatMonthLabel } from "@/utils/dashboardDates";
import { cn } from "@/utils/cn";
import { type DashboardViewMode, useDashboardState } from "../hooks/useDashboardState";
import { LAYERS, useEnabledLayers, useSidebarOpen } from "../lib/layers";
import { CalendarSync } from "./CalendarSync";
import { MiniCalendar } from "./MiniCalendar";
import { FocusTrayIndicator } from "./productivity/FocusTrayIndicator";
import "./Dashboard.css";

const VIEWS: { key: DashboardViewMode; label: string }[] = [
  { key: "day", label: "Day" },
  { key: "week", label: "Week" },
  { key: "month", label: "Month" },
  { key: "year", label: "Year" },
];

function formatDateDisplay(mode: DashboardViewMode, date: string): string {
  if (mode === "year") {
    return /^\d{4}$/.test(date) ? date : String(new Date().getFullYear());
  }
  // Day/Week/Month all expect YYYY-MM-DD.
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) return "—";
  if (mode === "day") return formatFullDate(date);
  if (mode === "month") return formatMonthLabel(date.slice(0, 7));
  // Week mode: "Apr 27 – May 3, 2026"
  const d = new Date(`${date}T00:00:00`);
  const end = new Date(d);
  end.setDate(end.getDate() + 6);
  return `${d.toLocaleDateString("en-US", { month: "short", day: "numeric" })} – ${end.toLocaleDateString(
    "en-US",
    { month: "short", day: "numeric", year: "numeric" },
  )}`;
}

interface PopoverPos {
  top: number;
  right: number;
}

function useClickOutside(
  ref: React.RefObject<HTMLElement | null>,
  onOutside: () => void,
  active: boolean,
) {
  useEffect(() => {
    if (!active) return;
    function handler(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) onOutside();
    }
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [active, onOutside, ref]);
}

export function DashboardTopbar() {
  const { mode, date, setMode, setDate, navigatePrev, navigateNext } = useDashboardState();

  const { sidebarOpen, toggleSidebar } = useSidebarOpen();
  const { enabled, toggle, reset } = useEnabledLayers();

  // Layers popover
  const [layersOpen, setLayersOpen] = useState(false);
  const layersTriggerRef = useRef<HTMLButtonElement | null>(null);
  const layersDropdownRef = useRef<HTMLDivElement | null>(null);
  const [layersPos, setLayersPos] = useState<PopoverPos>({ top: 0, right: 0 });
  useClickOutside(layersDropdownRef, () => setLayersOpen(false), layersOpen);

  const updateLayersPos = useCallback(() => {
    if (!layersTriggerRef.current) return;
    const rect = layersTriggerRef.current.getBoundingClientRect();
    setLayersPos({ top: rect.bottom + 4, right: window.innerWidth - rect.right });
  }, []);

  useEffect(() => {
    if (!layersOpen) return;
    updateLayersPos();
    window.addEventListener("resize", updateLayersPos);
    return () => window.removeEventListener("resize", updateLayersPos);
  }, [layersOpen, updateLayersPos]);

  // Mini-calendar popover
  const [calOpen, setCalOpen] = useState(false);
  const calTriggerRef = useRef<HTMLButtonElement | null>(null);
  const calDropdownRef = useRef<HTMLDivElement | null>(null);
  const [calPos, setCalPos] = useState<PopoverPos>({ top: 0, right: 0 });
  useClickOutside(calDropdownRef, () => setCalOpen(false), calOpen);

  const updateCalPos = useCallback(() => {
    if (!calTriggerRef.current) return;
    const rect = calTriggerRef.current.getBoundingClientRect();
    setCalPos({ top: rect.bottom + 4, right: window.innerWidth - rect.right });
  }, []);

  useEffect(() => {
    if (!calOpen) return;
    updateCalPos();
    window.addEventListener("resize", updateCalPos);
    return () => window.removeEventListener("resize", updateCalPos);
  }, [calOpen, updateCalPos]);

  const handleDateSelect = (iso: string) => {
    setDate(mode === "year" ? new Date(`${iso}T00:00:00`).getFullYear().toString() : iso);
    setCalOpen(false);
  };

  return (
    <div className="flex items-center gap-4 px-4 py-2 bg-transparent border-none rounded-none">
      <span className="text-[var(--fs-base)] font-medium text-text-strong whitespace-nowrap">
        {formatDateDisplay(mode, date)}
      </span>
      <FocusTrayIndicator />

      {/* View-pill switcher */}
      <div className="flex items-center bg-surface-hover rounded-full p-0.5">
        {VIEWS.map((v) => (
          <button
            key={v.key}
            type="button"
            onClick={() => setMode(v.key)}
            className={cn(
              "bg-transparent border-none px-3.5 py-1 rounded-full text-ui-xs font-medium text-text-muted cursor-pointer transition-colors duration-ui-fast ease-out hover:text-text-strong",
              mode === v.key && "bg-surface-active text-text-strong",
            )}
            aria-pressed={mode === v.key}
          >
            {v.label}
          </button>
        ))}
      </div>

      {/* Layers toggle */}
      <button
        ref={layersTriggerRef}
        type="button"
        onClick={() => setLayersOpen((v) => !v)}
        aria-haspopup="dialog"
        aria-expanded={layersOpen}
        aria-label="Toggle layers"
        title="Toggle layers"
        className={cn(
          "bg-transparent border-none p-1.5 rounded-full text-text-muted cursor-pointer transition-colors duration-ui-fast ease-out inline-flex items-center justify-center hover:text-text-strong hover:bg-surface-active",
          layersOpen && "bg-surface-active text-text-strong",
        )}
      >
        <Layers className="w-4 h-4" />
      </button>

      <CalendarSync />

      {/* Prev / date-picker / next */}
      <div className="flex items-center bg-surface-hover rounded-full p-0.5 ml-auto">
        <button
          type="button"
          onClick={navigatePrev}
          aria-label="Previous"
          className="bg-transparent border-none p-1.5 rounded-full text-text-muted cursor-pointer transition-colors duration-ui-fast ease-out inline-flex items-center justify-center hover:text-text-strong hover:bg-surface-active"
        >
          <ChevronLeft className="w-4 h-4" />
        </button>
        <button
          ref={calTriggerRef}
          type="button"
          onClick={() => setCalOpen((v) => !v)}
          aria-haspopup="dialog"
          aria-expanded={calOpen}
          aria-label="Pick date"
          title="Pick date"
          className={cn(
            "bg-transparent border-none p-1.5 rounded-full text-text-muted cursor-pointer transition-colors duration-ui-fast ease-out inline-flex items-center justify-center hover:text-text-strong hover:bg-surface-active",
            calOpen && "bg-surface-active text-text-strong",
          )}
        >
          <Calendar className="w-4 h-4" />
        </button>
        <button
          type="button"
          onClick={navigateNext}
          aria-label="Next"
          className="bg-transparent border-none p-1.5 rounded-full text-text-muted cursor-pointer transition-colors duration-ui-fast ease-out inline-flex items-center justify-center hover:text-text-strong hover:bg-surface-active"
        >
          <ChevronRight className="w-4 h-4" />
        </button>
      </div>

      {/* Sidebar toggle */}
      <button
        type="button"
        onClick={toggleSidebar}
        title={sidebarOpen ? "Hide summary" : "Show summary"}
        aria-label={sidebarOpen ? "Hide summary" : "Show summary"}
        className={cn(
          "bg-transparent border-none p-1.5 rounded-full text-text-muted cursor-pointer transition-colors duration-ui-fast ease-out inline-flex items-center justify-center hover:text-text-strong hover:bg-surface-active",
          sidebarOpen && "bg-surface-active text-text-strong",
        )}
      >
        <PanelRight className="w-4 h-4" />
      </button>

      {/* Layers popover */}
      {layersOpen &&
        createPortal(
          <div
            ref={layersDropdownRef}
            className="fixed z-ui-modal bg-surface-popover border border-border-subtle shadow-ds-popover rounded-ui-lg p-1.5 min-w-[180px]"
            style={{ top: layersPos.top, right: layersPos.right }}
          >
            {LAYERS.map((layer) => (
              <label
                key={layer.key}
                className="flex items-center gap-2 px-2.5 py-1.5 text-ui-xs text-text-muted cursor-pointer rounded-md transition-colors duration-ui-fast ease-out hover:bg-surface-hover"
              >
                <input
                  type="checkbox"
                  checked={enabled.has(layer.key)}
                  onChange={() => toggle(layer.key)}
                  style={{ accentColor: "var(--border-accent)", width: 12, height: 12 }}
                />
                <span
                  className="w-2 h-2 rounded-full inline-block"
                  style={{ backgroundColor: layer.color }}
                />
                {layer.label}
              </label>
            ))}
            <button
              type="button"
              onClick={reset}
              className="w-full text-left mt-1 px-2.5 py-1.5 text-ui-2xs text-text-muted bg-transparent border-none cursor-pointer rounded-md transition-colors duration-ui-fast ease-out hover:bg-surface-hover hover:text-text-strong"
            >
              Reset to defaults
            </button>
          </div>,
          document.body,
        )}

      {/* MiniCalendar popover */}
      {calOpen &&
        createPortal(
          <div
            ref={calDropdownRef}
            className="fixed z-ui-modal bg-surface-popover border border-border-subtle shadow-ds-popover rounded-ui-lg p-2.5"
            style={{ top: calPos.top, right: calPos.right }}
          >
            <MiniCalendar
              value={mode === "year" ? null : date}
              onSelect={handleDateSelect}
              showShortcuts={false}
            />
          </div>,
          document.body,
        )}
    </div>
  );
}
