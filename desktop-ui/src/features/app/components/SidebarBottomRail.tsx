import ScrollText from "lucide-react/dist/esm/icons/scroll-text";
import Settings from "lucide-react/dist/esm/icons/settings";
import User from "lucide-react/dist/esm/icons/user";
import X from "lucide-react/dist/esm/icons/x";
import { useEffect } from "react";
import {
  MenuTrigger,
  PopoverSurface,
} from "@/features/design-system/components/popover/PopoverPrimitives";
import { cn } from "@/utils/cn";
import { useMenuController } from "../hooks/useMenuController";

type SidebarBottomRailProps = {
  sessionPercent: number | null;
  weeklyPercent: number | null;
  sessionResetLabel: string | null;
  weeklyResetLabel: string | null;
  creditsLabel: string | null;
  showWeekly: boolean;
  onOpenSettings: () => void;
  onOpenDebug: () => void;
  showDebugButton: boolean;
  showAccountSwitcher: boolean;
  accountLabel: string;
  accountActionLabel: string;
  accountDisabled: boolean;
  accountSwitching: boolean;
  accountCancelDisabled: boolean;
  onSwitchAccount: () => void;
  onCancelSwitchAccount: () => void;
};

type UsageRowProps = {
  label: string;
  percent: number | null;
  resetLabel: string | null;
};

function UsageRow({ label, percent, resetLabel }: UsageRowProps) {
  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-baseline justify-between gap-3">
        <span className="text-text-stronger text-ui-xs font-semibold tracking-[0.01em]">{label}</span>
        <span className="text-text-stronger text-ui-xs font-bold tracking-[-0.01em]">{percent === null ? "--" : `${percent}%`}</span>
      </div>
      <div className="relative h-1 rounded-full bg-[color-mix(in_srgb,var(--surface-card-muted)_86%,transparent)] overflow-hidden" aria-hidden>
        <span
          className="block h-full rounded-full bg-gradient-to-r from-[rgba(120,235,190,0.92)] to-[rgba(100,200,255,0.92)] shadow-[0_0_12px_rgba(92,168,255,0.18)] transition-[width,opacity] duration-[180ms] ease-out"
          style={{ width: `${percent ?? 0}%` }}
        />
      </div>
      {resetLabel && <div className="text-text-muted text-[9px] leading-tight">{resetLabel}</div>}
    </div>
  );
}

export function SidebarBottomRail({
  sessionPercent,
  weeklyPercent,
  sessionResetLabel,
  weeklyResetLabel,
  creditsLabel,
  showWeekly,
  onOpenSettings,
  onOpenDebug,
  showDebugButton,
  showAccountSwitcher,
  accountLabel,
  accountActionLabel,
  accountDisabled,
  accountSwitching,
  accountCancelDisabled,
  onSwitchAccount,
  onCancelSwitchAccount,
}: SidebarBottomRailProps) {
  const accountMenu = useMenuController();
  const {
    isOpen: accountMenuOpen,
    containerRef: accountMenuRef,
    close: closeAccountMenu,
    toggle: toggleAccountMenu,
  } = accountMenu;

  useEffect(() => {
    if (!showAccountSwitcher) {
      closeAccountMenu();
    }
  }, [closeAccountMenu, showAccountSwitcher]);

  return (
    <div className="mt-1 pt-[10px] border-t border-border-subtle flex flex-col gap-[10px] [webkit-app-region:no-drag]">
      <div className="flex flex-col gap-2">
        <div className="flex items-baseline justify-between gap-[10px]">
          <div className="text-text-faint text-ui-2xs font-semibold tracking-[0.08em] uppercase">Usage</div>
          {creditsLabel && <div className="text-text-subtle text-ui-2xs max-w-[60%] overflow-hidden text-ellipsis whitespace-nowrap text-right">{creditsLabel}</div>}
        </div>
        <div className="flex flex-col gap-2">
          <UsageRow label="Session" percent={sessionPercent} resetLabel={sessionResetLabel} />
          {showWeekly && (
            <UsageRow label="Weekly" percent={weeklyPercent} resetLabel={weeklyResetLabel} />
          )}
        </div>
      </div>
      <div className={cn("flex flex-row items-stretch gap-[6px] w-full", !showAccountSwitcher && "justify-start")}>
        {showAccountSwitcher && (
          <div className="relative flex-1 min-w-0 w-auto" ref={accountMenuRef}>
            <MenuTrigger
              isOpen={accountMenuOpen}
              popupRole="dialog"
              className="ghost sidebar-labeled-button sidebar-account-trigger w-full min-w-0 justify-start h-[34px] px-[10px] py-1 rounded-[10px] border border-border-quiet bg-[color-mix(in_srgb,var(--surface-hover)_82%,transparent)] text-text-muted inline-flex items-center gap-2 whitespace-nowrap transition-colors duration-[120ms] ease-out hover:text-text-stronger hover:border-border-subtle hover:bg-surface-hover focus-visible:text-text-stronger focus-visible:border-border-subtle focus-visible:bg-surface-hover"
              activeClassName="is-open"
              onClick={toggleAccountMenu}
              aria-label="Account"
            >
              <span className="min-w-0 inline-flex items-center gap-2">
                <span
                  className="w-5 h-5 rounded-full inline-flex items-center justify-center shrink-0 text-text-stronger text-ui-2xs font-bold bg-gradient-to-br from-[rgba(120,235,190,0.28)] to-[rgba(100,200,255,0.26)]"
                  aria-hidden
                >
                  <User size={12} aria-hidden />
                </span>
                <span className="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-ui-xs font-semibold leading-none">Account</span>
              </span>
            </MenuTrigger>
            {accountMenuOpen && (
              <PopoverSurface
                className="absolute left-0 bottom-[calc(100%+8px)] min-w-[220px] p-[10px_12px] grid gap-2 z-[10]"
                role="dialog"
              >
                <div className="text-text-subtle text-ui-2xs uppercase tracking-[0.08em]">Account</div>
                <div className="text-text-stronger font-semibold text-ui-sm max-w-[220px] overflow-hidden text-ellipsis whitespace-nowrap">{accountLabel}</div>
                <div className="flex items-stretch gap-[6px]">
                  <button
                    type="button"
                    className="primary sidebar-account-action w-full justify-center text-ui-xs"
                    onClick={onSwitchAccount}
                    disabled={accountDisabled}
                    aria-busy={accountSwitching}
                  >
                    <span className="inline-flex items-center justify-center gap-[6px]">
                      {accountSwitching && (
                        <span
                          className="sidebar-account-spinner w-3 h-3 rounded-full border-2 border-[rgba(11,15,26,0.22)] border-t-[rgba(11,15,26,0.8)] animate-[spin_var(--ds-spinner-dur)_linear_infinite]"
                          aria-hidden
                        />
                      )}
                      <span>{accountActionLabel}</span>
                    </span>
                  </button>
                  {accountSwitching && (
                    <button
                      type="button"
                      className="secondary sidebar-account-cancel w-[34px] p-0 inline-flex items-center justify-center"
                      onClick={onCancelSwitchAccount}
                      disabled={accountCancelDisabled}
                      aria-label="Cancel account switch"
                      title="Cancel"
                    >
                      <X size={12} aria-hidden />
                    </button>
                  )}
                </div>
              </PopoverSurface>
            )}
          </div>
        )}
        <div className={cn("inline-flex flex-row items-stretch gap-[6px] w-auto", showAccountSwitcher ? "flex-1" : "flex-1 w-full")}>
          <button
            className="ghost sidebar-labeled-button sidebar-utility-button h-[34px] justify-start flex-1"
            type="button"
            onClick={onOpenSettings}
            aria-label="Open settings"
          >
            <span className="w-5 h-5 inline-flex items-center justify-center shrink-0" aria-hidden>
              <Settings size={14} aria-hidden />
            </span>
            <span className="text-ui-xs font-semibold leading-none">Settings</span>
          </button>
          {showDebugButton && (
            <button
              className="ghost sidebar-utility-button h-[34px] justify-start flex-1"
              type="button"
              onClick={onOpenDebug}
              aria-label="Open debug log"
            >
              <ScrollText size={14} aria-hidden />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
