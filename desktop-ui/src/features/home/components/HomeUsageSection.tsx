import ChevronLeft from "lucide-react/dist/esm/icons/chevron-left";
import ChevronRight from "lucide-react/dist/esm/icons/chevron-right";
import RefreshCw from "lucide-react/dist/esm/icons/refresh-cw";
import { useEffect, useState } from "react";
import type { AccountSnapshot, LocalUsageSnapshot, RateLimitSnapshot } from "@/types";
import { formatCount, formatDayLabel, formatDuration, formatWeekRange } from "../homeFormatters";
import type { HomeStatCard, UsageMetric, UsageWorkspaceOption } from "../homeTypes";
import { buildHomeUsageViewModel } from "../homeUsageViewModel";

type HomeUsageSectionProps = {
  accountInfo: AccountSnapshot | null;
  accountRateLimits: RateLimitSnapshot | null;
  isLoadingLocalUsage: boolean;
  localUsageError: string | null;
  localUsageSnapshot: LocalUsageSnapshot | null;
  onRefreshLocalUsage: () => void;
  onUsageMetricChange: (metric: UsageMetric) => void;
  onUsageWorkspaceChange: (workspaceId: string | null) => void;
  usageMetric: UsageMetric;
  usageShowRemaining: boolean;
  usageWorkspaceId: string | null;
  usageWorkspaceOptions: UsageWorkspaceOption[];
};

function HomeUsageCard({ card }: { card: HomeStatCard }) {
  return (
    <div className={card.compact ? "home-usage-card is-compact" : "home-usage-card"}>
      <div className="text-ui-xs uppercase tracking-[0.08em] text-text-faint">{card.label}</div>
      <div className="text-text-stronger flex items-end gap-2 leading-tight flex-wrap min-h-7 max-w-full overflow-hidden">
        <span className="text-2xl font-semibold tracking-tight whitespace-nowrap text-ellipsis overflow-hidden max-w-full">{card.value}</span>
        {card.suffix && <span className="text-ui-xs text-text-subtle uppercase tracking-[0.08em]">{card.suffix}</span>}
      </div>
      <div className="text-ui-sm text-text-subtle">{card.caption}</div>
    </div>
  );
}

export function HomeUsageSection({
  accountInfo,
  accountRateLimits,
  isLoadingLocalUsage,
  localUsageError,
  localUsageSnapshot,
  onRefreshLocalUsage,
  onUsageMetricChange,
  onUsageWorkspaceChange,
  usageMetric,
  usageShowRemaining,
  usageWorkspaceId,
  usageWorkspaceOptions,
}: HomeUsageSectionProps) {
  const [chartWeekOffset, setChartWeekOffset] = useState(0);
  const { accountCards, accountMeta, updatedLabel, usageCards, usageDays, usageInsights } =
    buildHomeUsageViewModel({
      accountInfo,
      accountRateLimits,
      localUsageSnapshot,
      usageMetric,
      usageShowRemaining,
    });

  const maxHistoricalWeekOffset = Math.max(0, Math.ceil(usageDays.length / 7) - 1);
  useEffect(() => {
    setChartWeekOffset((previous) => Math.min(previous, maxHistoricalWeekOffset));
  }, [maxHistoricalWeekOffset]);

  const chartWeekEnd = Math.max(0, usageDays.length - chartWeekOffset * 7);
  const chartWeekStart = Math.max(0, chartWeekEnd - 7);
  const chartDays = usageDays.slice(chartWeekStart, chartWeekEnd);
  const maxUsageValue = Math.max(
    1,
    ...chartDays.map((day) =>
      usageMetric === "tokens" ? day.totalTokens : (day.agentTimeMs ?? 0),
    ),
  );
  const canShowOlderWeek = chartWeekOffset < maxHistoricalWeekOffset;
  const canShowNewerWeek = chartWeekOffset > 0;
  const chartRangeLabel = formatWeekRange(chartDays);
  const chartRangeAriaLabel =
    chartDays.length > 0
      ? `Usage week ${chartDays[0]?.day} to ${chartDays[chartDays.length - 1]?.day}`
      : "Usage week";
  const showUsageSkeleton = isLoadingLocalUsage && !localUsageSnapshot;
  const showUsageEmpty = !isLoadingLocalUsage && !localUsageSnapshot;

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-baseline justify-between gap-3 min-w-0">
        <div className="text-ui-sm uppercase tracking-[0.08em] text-text-faint">Usage snapshot</div>
        <div className="inline-flex items-center flex-wrap justify-end gap-2 min-w-0">
          {updatedLabel && <div className="text-ui-sm text-text-subtle">{updatedLabel}</div>}
          <button
            type="button"
            className={isLoadingLocalUsage ? "home-usage-refresh is-loading" : "home-usage-refresh"}
            onClick={onRefreshLocalUsage}
            disabled={isLoadingLocalUsage}
            aria-label="Refresh usage"
            title="Refresh usage"
          >
            <RefreshCw
              className={
                isLoadingLocalUsage ? "home-usage-refresh-icon spinning" : "home-usage-refresh-icon"
              }
              aria-hidden
            />
          </button>
        </div>
      </div>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="inline-flex items-center gap-2 min-w-0">
          <span className="text-ui-xs uppercase tracking-[0.08em] text-text-faint">Workspace</span>
          <div className="inline-flex items-center px-2.5 py-0.5 rounded-full bg-surface-card border border-border-subtle min-w-0 max-w-full">
            <select
              className="appearance-none border-none bg-transparent text-text-stronger text-ui-sm py-1 pr-5 cursor-pointer max-w-[220px] overflow-hidden text-ellipsis whitespace-nowrap"
              value={usageWorkspaceId ?? ""}
              onChange={(event) => onUsageWorkspaceChange(event.target.value || null)}
              disabled={usageWorkspaceOptions.length === 0}
            >
              <option value="">All workspaces</option>
              {usageWorkspaceOptions.map((option) => (
                <option key={option.id} value={option.id}>
                  {option.label}
                </option>
              ))}
            </select>
          </div>
        </div>
        <div className="inline-flex items-center gap-2 min-w-0">
          <span className="text-ui-xs uppercase tracking-[0.08em] text-text-faint">View</span>
          <fieldset className="inline-flex items-center gap-0.5 p-0.5 rounded-full bg-surface-card border border-border-subtle" aria-label="Usage view">
            <button
              type="button"
              className={
                usageMetric === "tokens"
                  ? "home-usage-toggle-button is-active"
                  : "home-usage-toggle-button"
              }
              onClick={() => onUsageMetricChange("tokens")}
              aria-pressed={usageMetric === "tokens"}
            >
              Tokens
            </button>
            <button
              type="button"
              className={
                usageMetric === "time"
                  ? "home-usage-toggle-button is-active"
                  : "home-usage-toggle-button"
              }
              onClick={() => onUsageMetricChange("time")}
              aria-pressed={usageMetric === "time"}
            >
              Time
            </button>
          </fieldset>
        </div>
      </div>
      {showUsageSkeleton ? (
        <div className="home-usage-skeleton">
          <div className="grid grid-cols-4 gap-3">
            <div className="home-usage-card" key="skeleton-1">
              <span className="home-latest-skeleton h-3 w-16 rounded" />
              <span className="home-latest-skeleton h-7 w-20 rounded mt-1" />
            </div>
            <div className="home-usage-card" key="skeleton-2">
              <span className="home-latest-skeleton h-3 w-16 rounded" />
              <span className="home-latest-skeleton h-7 w-20 rounded mt-1" />
            </div>
            <div className="home-usage-card" key="skeleton-3">
              <span className="home-latest-skeleton h-3 w-16 rounded" />
              <span className="home-latest-skeleton h-7 w-20 rounded mt-1" />
            </div>
            <div className="home-usage-card" key="skeleton-4">
              <span className="home-latest-skeleton h-3 w-16 rounded" />
              <span className="home-latest-skeleton h-7 w-20 rounded mt-1" />
            </div>
          </div>
          <div className="home-usage-chart-card">
            <span className="home-latest-skeleton h-full w-full rounded-xl" />
          </div>
        </div>
      ) : showUsageEmpty ? (
        <div className="flex flex-col gap-1.5 p-3.5 rounded-xl border border-dashed border-border-subtle bg-surface-card">
          <div className="text-ui-sm font-semibold text-text-strong">No usage data yet</div>
          <div className="text-ui-xs text-text-subtle">
            Run a session to start tracking local usage.
          </div>
          {localUsageError && <div className="text-ui-xs text-status-error mt-1">{localUsageError}</div>}
        </div>
      ) : (
        <>
          <div className="grid grid-cols-4 gap-3">
            {usageCards.map((card) => (
              <HomeUsageCard card={card} key={card.label} />
            ))}
          </div>
          <div className="home-usage-chart-card">
            <div className="flex items-center justify-between gap-3 mb-3">
              <div
                className="text-ui-sm text-text-stronger tracking-wide"
                role="status"
                aria-label={chartRangeAriaLabel}
                aria-live="polite"
              >
                {chartRangeLabel}
              </div>
              <div className="inline-flex items-center gap-2">
                {canShowOlderWeek && (
                  <button
                    type="button"
                    className="w-7 h-7 rounded-full border border-border-subtle bg-white/[0.04] text-text-stronger inline-flex items-center justify-center cursor-pointer transition-all duration-120 hover:bg-surface-card-strong hover:border-border-strong hover:-translate-y-px p-0 disabled:opacity-40 disabled:cursor-default disabled:bg-white/[0.02] disabled:text-text-faint disabled:border-border-subtle disabled:translate-y-0"
                    onClick={() => setChartWeekOffset((current) => current + 1)}
                    aria-label="Show previous week"
                    title="Show previous week"
                  >
                    <ChevronLeft aria-hidden />
                  </button>
                )}
                <button
                  type="button"
                  className="w-7 h-7 rounded-full border border-border-subtle bg-white/[0.04] text-text-stronger inline-flex items-center justify-center cursor-pointer transition-all duration-120 hover:bg-surface-card-strong hover:border-border-strong hover:-translate-y-px p-0 disabled:opacity-40 disabled:cursor-default disabled:bg-white/[0.02] disabled:text-text-faint disabled:border-border-subtle disabled:translate-y-0"
                  onClick={() => setChartWeekOffset((current) => Math.max(0, current - 1))}
                  aria-label="Show next week"
                  title="Show next week"
                  disabled={!canShowNewerWeek}
                >
                  <ChevronRight aria-hidden />
                </button>
              </div>
            </div>
            <div className="grid grid-cols-7 items-end gap-2 h-[120px]">
              {chartDays.map((day) => {
                const value = usageMetric === "tokens" ? day.totalTokens : (day.agentTimeMs ?? 0);
                const height = Math.max(6, Math.round((value / maxUsageValue) * 100));
                const tooltip =
                  usageMetric === "tokens"
                    ? `${formatDayLabel(day.day)} · ${formatCount(day.totalTokens)} tokens`
                    : `${formatDayLabel(day.day)} · ${formatDuration(day.agentTimeMs ?? 0)} agent time`;
                return (
                  <div className="home-usage-bar" key={day.day} data-value={tooltip}>
                    <span className="home-usage-bar-fill" style={{ height: `${height}%` }} />
                    <span className="text-ui-2xs text-text-faint">{formatDayLabel(day.day)}</span>
                  </div>
                );
              })}
            </div>
          </div>
          <div className="grid grid-cols-3 gap-3">
            {usageInsights.map((card) => (
              <HomeUsageCard card={card} key={card.label} />
            ))}
          </div>
          <div className="flex flex-col gap-2">
            <div className="text-ui-sm uppercase tracking-[0.08em] text-text-faint inline-flex items-center gap-1.5">
              Top models
              {usageMetric === "time" && <span className="text-ui-2xs text-text-subtle normal-case tracking-wide">Tokens</span>}
            </div>
            <div className="flex flex-wrap gap-2 min-w-0">
              {localUsageSnapshot?.topModels?.length ? (
                localUsageSnapshot.topModels.map((model) => (
                  <span
                    className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-ui-sm text-text-stronger bg-white/[0.04] border border-border-subtle max-w-full min-w-0 break-words"
                    key={model.model}
                    title={`${model.model}: ${formatCount(model.tokens)} tokens`}
                  >
                    {model.model}
                    <span className="text-ui-2xs text-text-subtle">{model.sharePercent.toFixed(1)}%</span>
                  </span>
                ))
              ) : (
                <span className="text-ui-sm text-text-faint">No models yet</span>
              )}
            </div>
            {localUsageError && <div className="text-ui-xs text-status-error mt-1">{localUsageError}</div>}
          </div>
        </>
      )}
      {accountCards.length > 0 && (
        <div className="flex flex-col gap-3">
          <div className="flex items-baseline justify-between gap-3 min-w-0">
            <div className="text-ui-sm uppercase tracking-[0.08em] text-text-faint">Account limits</div>
            {accountMeta && (
              <div className="inline-flex items-center flex-wrap justify-end gap-2 min-w-0">
                <div className="text-ui-sm text-text-subtle">{accountMeta}</div>
              </div>
            )}
          </div>
          <div className="home-usage-grid home-account-grid">
            {accountCards.map((card) => (
              <HomeUsageCard card={card} key={card.label} />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
