import { Calendar, ChevronLeft, ChevronRight, Layers, PanelRight } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { formatFullDate, formatMonthLabel } from "@/utils/dashboardDates";
import { type DashboardViewMode, useDashboardState } from "../hooks/useDashboardState";
import { LAYERS, useEnabledLayers, useSidebarOpen } from "../lib/layers";
import { CalendarSync } from "./CalendarSync";
import { MiniCalendar } from "./MiniCalendar";

const VIEWS: { key: DashboardViewMode; label: string }[] = [
  { key: "day", label: "Day" },
  { key: "week", label: "Week" },
  { key: "month", label: "Month" },
  { key: "year", label: "Year" },
];

function formatDateDisplay(mode: DashboardViewMode, date: string): string {
  if (mode === "year") return date;
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
    <div className="dashboard__topbar">
      <span className="dashboard__topbar-date">{formatDateDisplay(mode, date)}</span>

      {/* View-pill switcher */}
      <div className="dashboard__view-switcher">
        {VIEWS.map((v) => (
          <button
            key={v.key}
            type="button"
            onClick={() => setMode(v.key)}
            className={`dashboard__view-pill${mode === v.key ? " dashboard__view-pill--active" : ""}`}
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
        className={`dashboard__icon-button${layersOpen ? " dashboard__icon-button--active" : ""}`}
      >
        <Layers />
      </button>

      <CalendarSync />

      {/* Prev / date-picker / next */}
      <div className="dashboard__nav-pills">
        <button
          type="button"
          onClick={navigatePrev}
          aria-label="Previous"
          className="dashboard__icon-button"
        >
          <ChevronLeft />
        </button>
        <button
          ref={calTriggerRef}
          type="button"
          onClick={() => setCalOpen((v) => !v)}
          aria-haspopup="dialog"
          aria-expanded={calOpen}
          aria-label="Pick date"
          title="Pick date"
          className={`dashboard__icon-button${calOpen ? " dashboard__icon-button--active" : ""}`}
        >
          <Calendar />
        </button>
        <button
          type="button"
          onClick={navigateNext}
          aria-label="Next"
          className="dashboard__icon-button"
        >
          <ChevronRight />
        </button>
      </div>

      {/* Sidebar toggle */}
      <button
        type="button"
        onClick={toggleSidebar}
        title={sidebarOpen ? "Hide summary" : "Show summary"}
        aria-label={sidebarOpen ? "Hide summary" : "Show summary"}
        className={`dashboard__icon-button${sidebarOpen ? " dashboard__icon-button--active" : ""}`}
      >
        <PanelRight />
      </button>

      {/* Layers popover */}
      {layersOpen &&
        createPortal(
          <div
            ref={layersDropdownRef}
            className="dashboard__popover"
            style={{ top: layersPos.top, right: layersPos.right }}
          >
            {LAYERS.map((layer) => (
              <label key={layer.key} className="dashboard__popover-item">
                <input
                  type="checkbox"
                  checked={enabled.has(layer.key)}
                  onChange={() => toggle(layer.key)}
                  style={{ accentColor: "var(--border-accent)", width: 12, height: 12 }}
                />
                <span
                  className="dashboard__layer-swatch"
                  style={{ backgroundColor: layer.color }}
                />
                {layer.label}
              </label>
            ))}
            <button type="button" onClick={reset} className="dashboard__popover-reset">
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
            className="dashboard__popover"
            style={{ top: calPos.top, right: calPos.right, padding: 10 }}
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
