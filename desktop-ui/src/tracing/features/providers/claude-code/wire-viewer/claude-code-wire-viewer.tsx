import type { WireEvent } from "@/tracing/lib/api";
import { ApiErrorCard } from "./cards/api-error-card";
import { AssistantTextCard } from "./cards/assistant-text-card";
import { CompactionCard } from "./cards/compaction-card";
import { OtherCard } from "./cards/other-card";
import { PrLinkCard } from "./cards/pr-link-card";
import { StatusUpdateCard } from "./cards/status-update-card";
import { ThinkingCard } from "./cards/thinking-card";
import { ToolCallCard } from "./cards/tool-call-card";
import { ToolResultCard } from "./cards/tool-result-card";
import { UserInputCard } from "./cards/user-input-card";

interface Props {
  events: WireEvent[];
  showMeta?: boolean;
  showOther?: boolean;
}

export function ClaudeCodeWireViewer({ events, showMeta = false, showOther = false }: Props) {
  return (
    <div className="cc-wire-viewer">
      {events
        .filter((e) => showMeta || !((e as { meta?: boolean }).meta ?? false))
        .map((e) => {
          const card = pickCard(e, showOther);
          return card ? <div key={e.index}>{card}</div> : null;
        })}
    </div>
  );
}

function pickCard(e: WireEvent, showOther: boolean): React.ReactNode {
  const t = e.type;
  const subtype = (e.payload as { subtype?: string }).subtype;

  if (t.endsWith(".thinking")) return <ThinkingCard event={e} />;
  if (t.endsWith(".text") && t.startsWith("assistant")) return <AssistantTextCard event={e} />;
  if (t.endsWith(".tool_use")) return <ToolCallCard event={e} />;
  if (t.endsWith(".tool_result")) return <ToolResultCard event={e} />;
  if (t.endsWith(".text") && t.startsWith("user")) return <UserInputCard event={e} />;
  if (t.endsWith(".image") && t.startsWith("user")) return <UserInputCard event={e} />;
  if (t === "system" && subtype === "compact_boundary") return <CompactionCard event={e} />;
  if (t === "system" && subtype === "api_error") return <ApiErrorCard event={e} />;
  if (
    t === "system" &&
    [
      "turn_duration",
      "stop_hook_summary",
      "away_summary",
      "local_command",
      "scheduled_task_fire",
    ].includes(subtype ?? "")
  ) {
    return <StatusUpdateCard event={e} />;
  }
  if (t === "pr-link") return <PrLinkCard event={e} />;
  if (t === "synthetic.TurnBegin") {
    const idx = (e.payload as { turnIndex?: number }).turnIndex;
    if (idx == null) return null;
    return <div className="cc-turn-divider">Turn {idx}</div>;
  }
  if (showOther) return <OtherCard event={e} />;
  return null;
}
