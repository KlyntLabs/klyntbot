import type { SessionContext } from "@shared/types";
import { useLocation } from "react-router";

/**
 * Derives a SessionContext from the current route path.
 * Returns null when no entity context applies (/chat, /launcher, /tray).
 */
export function usePageContext(): SessionContext | null {
  const { pathname, search } = useLocation();

  // /project/:id
  const projectMatch = pathname.match(/^\/project\/(.+)$/);
  if (projectMatch) return { entityKind: "project", entityId: projectMatch[1] };

  // /task/:id
  const taskMatch = pathname.match(/^\/task\/(.+)$/);
  if (taskMatch) return { entityKind: "task", entityId: taskMatch[1] };

  // /objective/:id
  const objMatch = pathname.match(/^\/objective\/(.+)$/);
  if (objMatch) return { entityKind: "objective", entityId: objMatch[1] };

  // /finance/budgets, /finance/investments, etc.
  const finSubMatch = pathname.match(/^\/finance\/(.+)$/);
  if (finSubMatch) return { entityKind: `finance.${finSubMatch[1]}` };

  // /finance (hub)
  if (pathname === "/finance") return { entityKind: "finance" };

  // / (tasks dashboard)
  if (pathname === "/") {
    const params = new URLSearchParams(search);
    const tab = params.get("tab");
    if (tab) return { entityKind: `tasks.${tab.toLowerCase()}` };
    return { entityKind: "tasks" };
  }

  // /notes
  if (pathname === "/notes") return { entityKind: "notes" };

  // /chat, /launcher, /tray → no entity context
  return null;
}
