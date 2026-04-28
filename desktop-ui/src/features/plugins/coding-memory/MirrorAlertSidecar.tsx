import { useEffect, useState } from "react";
import { fetchMirrorAlerts, actMirrorAlert } from "@/api/endpoints/codingMemory";
import { EffectivenessTrendsChart } from "./EffectivenessTrendsChart";

interface Alert {
  id: string;
  kind: string;
  severity: string;
  headline: string;
  payload: string;
  createdAt: string;
  dismissed: boolean;
}

interface Props { repo?: string; }

export function MirrorAlertSidecar({ repo }: Props) {
  const [alerts, setAlerts] = useState<Alert[]>([]);
  const [loading, setLoading] = useState(true);

  const load = () => {
    setLoading(true);
    fetchMirrorAlerts({ repo }).then((data) => {
      if (Array.isArray(data)) setAlerts(data as Alert[]);
    }).finally(() => setLoading(false));
  };

  useEffect(() => { load(); }, [repo]);

  const handleAction = async (id: string, action: "approve" | "reject" | "snooze") => {
    await actMirrorAlert(id, action);
    load();
  };

  if (loading) return <div className="cm-state cm-state--loading">Loading alerts…</div>;
  if (alerts.length === 0) return <div className="cm-state cm-state--empty">No mirror alerts.</div>;

  return (
    <section className="cm-mirror" aria-label="Mirror alerts">
      <h3 className="cm-mirror__title">Mirror Alerts ({alerts.length})</h3>
      <ul className="cm-mirror__list">
        {alerts.map((a) => (
          <li key={a.id} className="cm-mirror__row">
            <div className="cm-mirror__top">
              <span className={`cm-event-chip cm-event-chip--${severityColor(a.severity)}`}>{a.severity}</span>
              <span className="cm-mirror__headline">{a.headline}</span>
            </div>
            <div className="cm-mirror__actions">
              <button type="button" className="cm-mirror__btn" onClick={() => handleAction(a.id, "approve")}>Approve</button>
              <button type="button" className="cm-mirror__btn" onClick={() => handleAction(a.id, "reject")}>Reject</button>
              <button type="button" className="cm-mirror__btn" onClick={() => handleAction(a.id, "snooze")}>Snooze</button>
            </div>
            {a.kind === "patternEffectivenessDrop" && (
              <EffectivenessTrendsChart patternId={a.id} />
            )}
          </li>
        ))}
      </ul>
    </section>
  );
}

function severityColor(severity: string): string {
  switch (severity) {
    case "error": return "neutral";
    case "warn": return "amber";
    case "info": return "neutral";
    default: return "neutral";
  }
}
