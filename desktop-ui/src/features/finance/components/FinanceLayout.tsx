import { useLocation, useNavigate } from "react-router";
import type { CurrencyDisplayMode } from "../hooks/useCurrencyDisplayMode";
import { CurrencyToggle } from "./CurrencyToggle";
import { PrivacyToggle } from "./PrivacyToggle";

interface FinanceLayoutProps {
  children: React.ReactNode;
  hidden?: boolean;
  onTogglePrivacy?: () => void;
  currencyMode?: CurrencyDisplayMode;
  currencies?: string[];
  onSelectCurrency?: (mode: CurrencyDisplayMode) => void;
}

const subNav = [
  { label: "Dashboard", path: "/finance" },
  { label: "Cash Flow", path: "/finance/cashflow" },
  { label: "Investments", path: "/finance/investments" },
  { label: "Goals", path: "/finance/goals" },
  { label: "Liabilities", path: "/finance/liabilities" },
];

export function FinanceLayout({
  children,
  hidden,
  onTogglePrivacy,
  currencyMode,
  currencies,
  onSelectCurrency,
}: FinanceLayoutProps) {
  const navigate = useNavigate();
  const location = useLocation();

  const currentPath = location.pathname;

  return (
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
        {onTogglePrivacy != null && hidden != null && (
          <PrivacyToggle hidden={hidden} onToggle={onTogglePrivacy} />
        )}
        {currencyMode && currencies && onSelectCurrency && (
          <CurrencyToggle mode={currencyMode} currencies={currencies} onSelect={onSelectCurrency} />
        )}
      </div>

      {/* Content — no glass wrapper, cards float on background */}
      <div className="flex-1 overflow-y-auto p-4">{children}</div>
    </div>
  );
}
