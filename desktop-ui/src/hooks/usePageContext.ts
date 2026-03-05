import { useLocation } from "react-router";
import type { SessionContext } from "../lib/types";

/**
 * Derives a SessionContext from the current route path.
 * Returns null when no entity context applies (dashboard, /chat).
 */
export function usePageContext(): SessionContext | null {
  const { pathname } = useLocation();

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

  // / (dashboard), /chat, /launcher, /tray → no entity context
  return null;
}
