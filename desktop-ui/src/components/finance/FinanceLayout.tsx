import { useState } from "react";
import { useLocation, useNavigate } from "react-router";
import type { SidebarItem } from "../../lib/types";
import { Sidebar } from "../layout/Sidebar";

interface FinanceLayoutProps {
  children: React.ReactNode;
  onRefresh?: () => void;
}

const subNav = [
  { label: "Dashboard", path: "/finance" },
  { label: "Accounts", path: "/finance/accounts" },
  { label: "Transactions", path: "/finance/transactions" },
  { label: "Budgets", path: "/finance/budgets" },
  { label: "Investments", path: "/finance/investments" },
  { label: "Goals", path: "/finance/goals" },
  { label: "Liabilities", path: "/finance/liabilities" },
];

export function FinanceLayout({ children }: FinanceLayoutProps) {
  const navigate = useNavigate();
  const location = useLocation();
  const [activeSidebar, setActiveSidebar] = useState<SidebarItem>("Finance");

  const currentPath = location.pathname;

  return (
    <div className="h-screen w-screen bg-background text-primary flex gap-2 p-2 overflow-hidden">
      <Sidebar
        active={activeSidebar}
        onNavigate={(item) => {
          setActiveSidebar(item);
          if (item === "Tasks") navigate("/");
          if (item === "Chat") navigate("/chat");
        }}
      />

      <div className="flex-1 flex flex-col gap-2 overflow-hidden">
        {/* Floating glass toolbar */}
        <div className="h-12 flex items-center px-2 shrink-0">
          <div className="flex-1 flex items-center gap-1.5" role="tablist">
            {subNav.map((item) => {
              const isActive = currentPath === item.path;
              return (
                <button
                  type="button"
                  key={item.path}
                  role="tab"
                  aria-selected={isActive}
                  onClick={() => navigate(item.path)}
                  className={`flex-1 py-2 rounded-xl text-[13px] font-light transition-all duration-200 ${
                    isActive
                      ? "glass-button-active text-primary"
                      : "text-muted hover:text-secondary hover:bg-white/[0.04]"
                  }`}
                >
                  {item.label}
                </button>
              );
            })}
          </div>
        </div>

        {/* Content — no glass wrapper, cards float on background */}
        <div className="flex-1 overflow-y-auto p-4">{children}</div>
      </div>
    </div>
  );
}
