import { Code } from "lucide-react";
import { NavLink, Outlet } from "react-router-dom";

const tabs = [
  { to: "cli-health", label: "CLI Health" },
  { to: "session-replay", label: "Session Replay" },
  { to: "memory", label: "Memory Browser" },
  { to: "activity", label: "Activity Timeline" },
  { to: "cost", label: "Cost Tracker" },
  { to: "sensitivity", label: "Sensitivity Inspector" },
  { to: "recall-log", label: "Recall Log" },
  { to: "mirror-alerts", label: "Mirror Alerts" },
  { to: "pattern-effectiveness", label: "Pattern Effectiveness" },
  { to: "reforge-diff", label: "Reforge Diff" },
];

export function CodingMemoryLayout() {
  return (
    <div className="flex h-full">
      <nav className="w-56 glass-sidebar flex flex-col py-3">
        <div className="px-3 mb-2">
          <span className="text-[11px] font-medium text-dim uppercase tracking-wider px-1">
            Coding Memory
          </span>
        </div>
        <div className="flex flex-col gap-0.5 px-2">
          {tabs.map((t) => {
            return (
              <NavLink
                key={t.to}
                to={t.to}
                end
                className={({ isActive }) =>
                  `flex items-center gap-2.5 px-2.5 py-1.5 rounded-xl text-[13px] font-light transition-all duration-200 text-left ${
                    isActive
                      ? "glass-button-active text-foreground"
                      : "text-muted-foreground hover:bg-accent hover:text-foreground"
                  }`
                }
              >
                <Code className="size-4 flex-shrink-0" strokeWidth={1.5} />
                {t.label}
              </NavLink>
            );
          })}
        </div>
      </nav>
      <main className="flex-1 overflow-auto">
        <Outlet />
      </main>
    </div>
  );
}
