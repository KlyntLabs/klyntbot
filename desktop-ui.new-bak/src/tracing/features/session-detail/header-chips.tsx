import type { HeaderLayoutResponse, SessionSummary } from "@/tracing/lib/api";

interface Props {
  chips: HeaderLayoutResponse["chips"];
  stats: SessionSummary;
  model?: string;
}

export function HeaderChips({ chips, stats, model }: Props) {
  return (
    <div className="cc-header-chips">
      {chips.map((c) => (
        <span key={c} className="cc-header-chip">
          {label(c)}
          <strong>{value(c, stats, model)}</strong>
        </span>
      ))}
    </div>
  );
}

function label(c: string): string {
  switch (c) {
    case "turns":
      return "Turns";
    case "steps":
      return "Steps";
    case "messages":
      return "Msgs";
    case "toolCalls":
      return "Tools";
    case "errors":
      return "Errors";
    case "compactions":
      return "Compacts";
    case "agents":
      return "Agents";
    case "duration":
      return "Dur";
    case "tokens":
      return "Tokens";
    case "cacheHitPct":
      return "Cache%";
    case "model":
      return "Model";
    default:
      return c;
  }
}

function value(c: string, s: SessionSummary, model?: string): string {
  switch (c) {
    case "turns":
      return String(s.turns);
    case "steps":
      return String(s.steps);
    case "toolCalls":
      return String(s.tool_calls);
    case "errors":
      return String(s.errors);
    case "compactions":
      return String(s.compactions);
    case "duration":
      return formatDur(s.duration_sec);
    case "tokens":
      return `${fmt(s.input_tokens)} / ${fmt(s.output_tokens)}`;
    case "model":
      return model ?? "—";
    default:
      return "—";
  }
}

function fmt(n: number): string {
  if (n > 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n > 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

function formatDur(sec: number): string {
  const m = Math.floor(sec / 60);
  const s = Math.floor(sec % 60);
  return m > 0 ? `${m}m ${s}s` : `${s}s`;
}
