import { Calendar, ChevronLeft, ChevronRight } from "lucide-react";
import { type ReactNode, useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useLocation, useNavigate, useParams } from "react-router";
import { useClickOutside } from "../../hooks/useClickOutside";
import { todayISO, toLocalISO } from "../../lib/dates";
import { cn } from "../../lib/utils";
import { MiniCalendar } from "../tasks/editors/MiniCalendar";

type ViewMode = "day" | "week" | "month" | "year";

function getViewMode(pathname: string): ViewMode {
  if (pathname.startsWith("/week/")) return "week";
  if (pathname.startsWith("/month/")) return "month";
  if (pathname.startsWith("/year/")) return "year";
  return "day";
}

function formatDateDisplay(mode: ViewMode, param: string): string {
  if (mode === "year") return param;
  const date = new Date(`${param}T00:00:00`);
  if (mode === "day") {
    return date.toLocaleDateString("en-US", {
      weekday: "long",
      month: "long",
      day: "numeric",
      year: "numeric",
    });
  }
  if (mode === "week") {
    const end = new Date(date);
    end.setDate(end.getDate() + 6);
    return `${date.toLocaleDateString("en-US", { month: "short", day: "numeric" })} – ${end.toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric" })}`;
  }
  return date.toLocaleDateString("en-US", { month: "long", year: "numeric" });
}

export function DashboardLayout({ children }: { children: ReactNode }) {
  const navigate = useNavigate();
  const location = useLocation();
  const params = useParams<{ date?: string; year?: string }>();
  const mode = getViewMode(location.pathname);
  const dateParam = params.date || params.year || todayISO();

  const navigateToView = (view: ViewMode) => {
    const today = todayISO();
    switch (view) {
      case "day":
        navigate(`/day/${today}`);
        break;
      case "week":
        navigate(`/week/${today}`);
        break;
      case "month":
        navigate(`/month/${today}`);
        break;
      case "year":
        navigate(`/year/${new Date().getFullYear()}`);
        break;
    }
  };

  const navigateBy = (dir: 1 | -1) => {
    const d = new Date(`${dateParam}T00:00:00`);
    switch (mode) {
      case "day":
        d.setDate(d.getDate() + dir);
        break;
      case "week":
        d.setDate(d.getDate() + 7 * dir);
        break;
      case "month":
        d.setMonth(d.getMonth() + dir);
        break;
      case "year":
        d.setFullYear(d.getFullYear() + dir);
        break;
    }
    const iso = mode === "year" ? String(d.getFullYear()) : toLocalISO(d);
    navigate(`/${mode}/${iso}`);
  };

  // Calendar picker popover
  const [calOpen, setCalOpen] = useState(false);
  const calTriggerRef = useRef<HTMLButtonElement>(null);
  const calDropdownRef = useRef<HTMLDivElement>(null);
  const [calPos, setCalPos] = useState({ top: 0, right: 0 });

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
    if (mode === "year") {
      const year = new Date(`${iso}T00:00:00`).getFullYear();
      navigate(`/year/${year}`);
    } else {
      navigate(`/${mode}/${iso}`);
    }
    setCalOpen(false);
  };

  const views: { key: ViewMode; label: string }[] = [
    { key: "day", label: "Day" },
    { key: "week", label: "Week" },
    { key: "month", label: "Month" },
    { key: "year", label: "Year" },
  ];

  return (
    <div className="flex-1 flex flex-col gap-2 min-w-0">
      {/* Top bar */}
      <div className="glass-card px-4 py-2 flex items-center gap-4">
        {/* Date label */}
        <span className="text-sm font-medium text-primary whitespace-nowrap">
          {formatDateDisplay(mode, dateParam)}
        </span>

        {/* View switcher pill group */}
        <div className="flex items-center rounded-full bg-white/[0.06] p-0.5">
          {views.map((v) => (
            <button
              key={v.key}
              type="button"
              onClick={() => navigateToView(v.key)}
              className={cn(
                "px-3.5 py-1 rounded-full text-xs font-medium transition-all",
                mode === v.key ? "bg-white/[0.12] text-primary" : "text-muted hover:text-secondary",
              )}
            >
              {v.label}
            </button>
          ))}
        </div>

        {/* Nav pill group */}
        <div className="flex items-center rounded-full bg-white/[0.06] p-0.5 ml-auto">
          <button
            type="button"
            onClick={() => navigateBy(-1)}
            className="p-1.5 rounded-full text-muted hover:text-secondary hover:bg-white/[0.08] transition-colors"
          >
            <ChevronLeft className="w-4 h-4" />
          </button>
          <button
            ref={calTriggerRef}
            type="button"
            onClick={() => setCalOpen(!calOpen)}
            aria-haspopup="dialog"
            aria-expanded={calOpen}
            className={cn(
              "p-1.5 rounded-full text-muted hover:text-secondary hover:bg-white/[0.08] transition-colors",
              calOpen && "bg-white/[0.08] text-secondary",
            )}
            title="Pick date"
          >
            <Calendar className="w-4 h-4" />
          </button>
          <button
            type="button"
            onClick={() => navigateBy(1)}
            className="p-1.5 rounded-full text-muted hover:text-secondary hover:bg-white/[0.08] transition-colors"
          >
            <ChevronRight className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* Calendar picker popover */}
      {calOpen &&
        createPortal(
          <div
            ref={calDropdownRef}
            className="fixed z-[9999] glass-dropdown"
            style={{ top: calPos.top, right: calPos.right }}
          >
            <MiniCalendar
              value={mode === "year" ? null : dateParam}
              onSelect={handleDateSelect}
              showShortcuts={false}
            />
          </div>,
          document.body,
        )}

      {/* Content */}
      <div className="flex-1 overflow-hidden">{children}</div>
    </div>
  );
}
