import type { TraceEvent } from "../types";

interface Props {
  event: TraceEvent;
}

export function StatusUpdateCard({ event }: Props) {
  const p = event.payload as any;
  const ctxPct = ((p?.context_usage ?? 0) * 100).toFixed(1);
  const ctxTok = p?.context_tokens ?? 0;
  const maxCtx = p?.max_context_tokens ?? 0;
  const u = p?.token_usage ?? {};
  const inTok = (u.input_other ?? 0) + (u.input_cache_read ?? 0) + (u.input_cache_creation ?? 0);
  const cacheRead = u.input_cache_read ?? 0;
  const cachePct = inTok > 0 ? ((cacheRead / inTok) * 100).toFixed(0) : "0";
  return (
    <div className="tracing-evcard tracing-evcard--status">
      ctx: {ctxPct}% · {(ctxTok / 1000).toFixed(1)}k / {(maxCtx / 1000).toFixed(0)}k tokens · in:{" "}
      {(inTok / 1000).toFixed(1)}k ({cachePct}% cache) · out: {(u.output ?? 0) / 1000}k
    </div>
  );
}
