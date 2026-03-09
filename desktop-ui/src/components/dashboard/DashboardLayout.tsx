import { ChevronLeft, ChevronRight } from "lucide-react";
import type { ReactNode } from "react";
import { useLocation, useNavigate, useParams } from "react-router";
import { todayISO, toLocalISO } from "../../lib/dates";
import { cn } from "../../lib/utils";

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

  const navigateToday = () => {
    const iso = mode === "year" ? String(new Date().getFullYear()) : todayISO();
    navigate(`/${mode}/${iso}`);
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
      <div className="glass-card px-4 py-2 flex items-center justify-between">
        <div className="flex items-center gap-2">
          {views.map((v) => (
            <button
              key={v.key}
              type="button"
              onClick={() => navigateToView(v.key)}
              className={cn(
                "px-3 py-1 rounded-lg text-xs font-medium transition-all",
                mode === v.key
                  ? "glass-button-active text-brand"
                  : "text-muted hover:text-secondary hover:bg-white/[0.05]",
              )}
            >
              {v.label}
            </button>
          ))}
        </div>

        <span className="text-sm font-medium text-primary">
          {formatDateDisplay(mode, dateParam)}
        </span>

        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={() => navigateBy(-1)}
            className="p-1 rounded hover:bg-white/[0.05] text-muted hover:text-secondary"
          >
            <ChevronLeft className="w-4 h-4" />
          </button>
          <button
            type="button"
            onClick={navigateToday}
            className="px-2 py-1 rounded text-xs text-muted hover:text-secondary hover:bg-white/[0.05]"
          >
            Today
          </button>
          <button
            type="button"
            onClick={() => navigateBy(1)}
            className="p-1 rounded hover:bg-white/[0.05] text-muted hover:text-secondary"
          >
            <ChevronRight className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-hidden">{children}</div>
    </div>
  );
}
