import { ExternalLink, X } from "lucide-react";
import { dashboardIntelligenceQuery, productivityTodayQuery } from "@/api/endpoints/dashboard";
import type {
  DashboardIntelligenceResponse,
  ProductivitySummaryResponse,
  TimelineEntry,
  TimelineSummary,
} from "@/bindings";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { formatHumanDuration, TZ_OFFSET_MINS, todayISO } from "@/utils/dashboardDates";

interface Props {
  summary: TimelineSummary | null;
  selectedEntry: TimelineEntry | null;
  onClose: () => void;
  date?: string;
}

export function SummaryPanel({ summary, selectedEntry, onClose, date }: Props) {
  if (selectedEntry) {
    return <EntryDetail entry={selectedEntry} onClose={onClose} />;
  }
  if (!summary) return null;
  return <DaySummary summary={summary} date={date || todayISO()} />;
}

function DaySummary({ summary, date }: { summary: TimelineSummary; date: string }) {
  const { data: ps } = useTauriQuery<ProductivitySummaryResponse | null>({
    queryKey: qk.dashboard.productivityToday(date),
    queryFn: () => productivityTodayQuery(),
    fallback: null,
  });
  const { data: intel } = useTauriQuery<DashboardIntelligenceResponse | null>({
    queryKey: qk.dashboard.intelligence(date),
    queryFn: () => dashboardIntelligenceQuery(date, TZ_OFFSET_MINS),
    fallback: null,
  });

  const hasProductivity = ps != null && ps.totalActiveSecs > 0;
  const productivePct = hasProductivity
    ? Math.round((ps.productiveSecs / ps.totalActiveSecs) * 100)
    : 0;

  return (
    <aside className="dashboard__summary-panel">
      {/* Active time + productive split */}
      {hasProductivity ? (
        <section className="dashboard__summary-section">
          <div className="dashboard__summary-active">
            <span className="dashboard__summary-active-time">
              {formatHumanDuration(ps.totalActiveSecs)}
            </span>
            <span className="dashboard__summary-dim">active</span>
          </div>
          <div className="dashboard__summary-bar">
            {ps.productiveSecs > 0 && (
              <div
                className="dashboard__summary-bar-seg dashboard__summary-bar-seg--productive"
                style={{ width: `${(ps.productiveSecs / ps.totalActiveSecs) * 100}%` }}
              />
            )}
            {ps.neutralSecs > 0 && (
              <div
                className="dashboard__summary-bar-seg dashboard__summary-bar-seg--neutral"
                style={{ width: `${(ps.neutralSecs / ps.totalActiveSecs) * 100}%` }}
              />
            )}
            {ps.distractingSecs > 0 && (
              <div
                className="dashboard__summary-bar-seg dashboard__summary-bar-seg--distracting"
                style={{ width: `${(ps.distractingSecs / ps.totalActiveSecs) * 100}%` }}
              />
            )}
          </div>
          <div className="dashboard__summary-productive-pct">{productivePct}% productive</div>
          <div className="dashboard__summary-metrics">
            <ScoreBar label="Deep focus" value={ps.productiveSecs / ps.totalActiveSecs} />
            <ScoreBar label="Quality" value={ps.avgSessionQuality ?? 0} />
            <ScoreBar
              label="Low distraction"
              value={1 - ps.distractingSecs / Math.max(ps.totalActiveSecs, 1)}
            />
          </div>
          {ps.deepWorkBlocks > 0 && (
            <div className="dashboard__summary-stat-row">
              <span>
                {ps.deepWorkBlocks} deep work block{ps.deepWorkBlocks !== 1 ? "s" : ""}
              </span>
              <span>{formatHumanDuration(ps.deepWorkSecs)}</span>
            </div>
          )}
          {ps.avgRecoverySecs != null && (
            <div className="dashboard__summary-stat-row">
              <span>Avg recovery</span>
              <span>{Math.round(ps.avgRecoverySecs)}s</span>
            </div>
          )}
        </section>
      ) : (
        <section className="dashboard__summary-section">
          <div className="dashboard__summary-active">
            <span className="dashboard__summary-active-time">
              {formatHumanDuration(summary.totalTrackedSecs)}
            </span>
            <span className="dashboard__summary-dim">tracked</span>
          </div>
        </section>
      )}

      {intel?.focusRecommendation && (
        <p className="dashboard__summary-recommendation">{intel.focusRecommendation}</p>
      )}

      {hasProductivity && ps.topApps.length > 0 && (
        <section className="dashboard__summary-section">
          <h4 className="dashboard__summary-heading">Top Apps</h4>
          <TopAppsChart apps={ps.topApps} />
        </section>
      )}

      {summary.topApps.length > 0 && !hasProductivity && (
        <section className="dashboard__summary-section">
          <h4 className="dashboard__summary-heading">Top Apps</h4>
          <TopAppsChart
            apps={summary.topApps.map((a) => ({
              appName: a.appName,
              durationSecs: a.durationSecs,
              category: null,
            }))}
          />
        </section>
      )}

      {intel && (intel.patterns.length > 0 || intel.nudges.length > 0) && (
        <section className="dashboard__summary-section">
          <h4 className="dashboard__summary-heading">Insights</h4>
          <ul className="dashboard__summary-insights">
            {intel.patterns.map((p) => (
              <li key={`p-${p}`}>{p}</li>
            ))}
            {intel.nudges.map((n) => (
              <li key={`n-${n.nudgeType}-${n.message}`}>{n.message}</li>
            ))}
          </ul>
        </section>
      )}

      {ps?.aiSummary && (
        <section className="dashboard__summary-aibox">
          <p>{ps.aiSummary}</p>
        </section>
      )}
    </aside>
  );
}

function ScoreBar({ label, value }: { label: string; value: number }) {
  const pct = Math.max(0, Math.min(1, value)) * 100;
  return (
    <div className="dashboard__summary-score-bar">
      <span className="dashboard__summary-score-label">{label}</span>
      <div className="dashboard__summary-score-track">
        <div className="dashboard__summary-score-fill" style={{ width: `${pct}%` }} />
      </div>
      <span className="dashboard__summary-score-value">{Math.round(pct)}</span>
    </div>
  );
}

function TopAppsChart({
  apps,
}: {
  apps: { appName: string; durationSecs: number; category?: string | null }[];
}) {
  const maxSecs = apps[0]?.durationSecs ?? 1;
  return (
    <div className="dashboard__summary-apps">
      {apps.slice(0, 5).map((app) => {
        const pct = maxSecs > 0 ? (app.durationSecs / maxSecs) * 100 : 0;
        return (
          <div key={app.appName} className="dashboard__summary-app-row">
            <span className="dashboard__summary-app-name" title={app.appName}>
              {app.appName}
            </span>
            <div className="dashboard__summary-app-track">
              <div
                className="dashboard__summary-app-fill"
                style={{ width: `${Math.max(pct, 4)}%` }}
              />
            </div>
            <span className="dashboard__summary-app-dur">
              {formatHumanDuration(app.durationSecs)}
            </span>
          </div>
        );
      })}
    </div>
  );
}

function EntryDetail({ entry, onClose }: { entry: TimelineEntry; onClose: () => void }) {
  return (
    <aside className="dashboard__summary-panel">
      <div className="dashboard__summary-detail-header">
        <h3 className="dashboard__summary-heading">Details</h3>
        <button
          type="button"
          onClick={onClose}
          className="dashboard__summary-close"
          aria-label="Close details"
        >
          <X aria-hidden />
        </button>
      </div>

      <div className="dashboard__summary-entry-title">
        <span
          className="dashboard__summary-entry-swatch"
          style={{ backgroundColor: entry.color }}
        />
        <span>{entry.title}</span>
      </div>

      {entry.description && <p className="dashboard__summary-entry-desc">{entry.description}</p>}

      <div className="dashboard__summary-entry-meta">
        <div>Started: {new Date(entry.startedAt).toLocaleTimeString()}</div>
        {entry.endedAt && <div>Ended: {new Date(entry.endedAt).toLocaleTimeString()}</div>}
        {entry.durationSecs != null && entry.durationSecs > 0 && (
          <div>Duration: {formatHumanDuration(entry.durationSecs)}</div>
        )}
        <div className="dashboard__summary-entry-source">Source: {entry.source}</div>
      </div>

      {entry.entityRoute && (
        <a
          href={entry.entityRoute}
          className="dashboard__summary-entry-link"
          onClick={(e) => e.preventDefault()}
        >
          <ExternalLink aria-hidden />
          <span>Open {entry.source}</span>
        </a>
      )}
    </aside>
  );
}
