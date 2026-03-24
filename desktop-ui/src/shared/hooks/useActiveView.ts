import { useMutation } from "@shared/hooks/useMutation";
import { useEffect, useRef } from "react";
import { useLocation } from "react-router";

interface ActiveViewParams {
  dashboard: string;
  focusedEntity?: string | null;
  description?: string | null;
}

/**
 * Pushes the active desktop view to the backend on route changes.
 * The agent's query rewriter uses this to enrich vague queries with
 * what the user is currently looking at (Moment 6: dashboard + chat synergy).
 */
export function useActiveView() {
  const { pathname, search } = useLocation();
  const { mutate: setActive } = useMutation<void, ActiveViewParams>("view_set_active", "params");
  const { mutate: clearActive } = useMutation<void, Record<string, never>>("view_clear_active");
  const lastRef = useRef<string>("");

  useEffect(() => {
    const key = `${pathname}?${search}`;
    if (key === lastRef.current) return;
    lastRef.current = key;

    const view = deriveActiveView(pathname, search);
    if (view) {
      setActive(view);
    } else {
      clearActive({});
    }
  }, [pathname, search, setActive, clearActive]);
}

const FINANCE_SUB_LABELS: Record<string, string> = {
  cashflow: "Cash flow analysis",
  investments: "Investment portfolio",
  targets: "Savings and allocation targets",
};

function deriveActiveView(pathname: string, search: string): ActiveViewParams | null {
  // Finance views
  if (pathname === "/finance")
    return { dashboard: "finance", description: "Finance overview dashboard" };
  const finSub = pathname.match(/^\/finance\/(.+)$/);
  if (finSub) {
    const sub = finSub[1];
    return {
      dashboard: "finance",
      focusedEntity: sub,
      description: FINANCE_SUB_LABELS[sub] ?? `Finance ${sub}`,
    };
  }

  // Task views
  if (pathname === "/" || pathname.startsWith("/tasks")) {
    const params = new URLSearchParams(search);
    const tab = params.get("tab");
    return {
      dashboard: "tasks",
      focusedEntity: tab,
      description: tab ? `Tasks ${tab} view` : "Tasks overview",
    };
  }

  // Project detail
  const projectMatch = pathname.match(/^\/project\/(.+?)(?:\/|$)/);
  if (projectMatch)
    return {
      dashboard: "projects",
      focusedEntity: projectMatch[1],
      description: "Project detail view",
    };

  // Projects list
  if (pathname === "/projects") return { dashboard: "projects", description: "Projects list" };

  // Notes / Knowledge base
  if (pathname.startsWith("/notes")) return { dashboard: "notes", description: "Knowledge base" };

  // Learning
  if (pathname.startsWith("/learn"))
    return { dashboard: "learning", description: "Learning and review" };

  // Coaching
  if (pathname.startsWith("/coaching"))
    return { dashboard: "coaching", description: "Coaching overview" };

  // Dashboard (day/week/month/year views)
  if (
    pathname.startsWith("/day/") ||
    pathname.startsWith("/week/") ||
    pathname.startsWith("/month/") ||
    pathname.startsWith("/year/")
  )
    return { dashboard: "dashboard", description: "Daily planner" };

  // OKR / Objectives
  const objMatch = pathname.match(/^\/objective\/(.+)$/);
  if (objMatch)
    return {
      dashboard: "okr",
      focusedEntity: objMatch[1],
      description: "Objective detail",
    };

  // Automations
  if (pathname === "/automations")
    return { dashboard: "automations", description: "Automations overview" };

  // Chat, launcher, tray, settings — no view context
  return null;
}
