import type { WireEventDto } from "./types";

export interface EventMeta {
  nestLevel: number;
  linkedToolName?: string;
  linkedToolCallId?: string;
}

export function buildToolGrouping(events: WireEventDto[]): Map<number, EventMeta> {
  const meta = new Map<number, EventMeta>();
  const active: { id: string; name: string }[] = [];
  const names = new Map<string, string>();
  const peek = () => (active.length ? active[active.length - 1] : undefined);

  events.forEach((e, i) => {
    const p = (e.payloadDecoded ?? {}) as Record<string, unknown> & {
      id?: unknown;
      function?: { name?: unknown };
      tool_name?: unknown;
      tool_call_id?: unknown;
    };
    if (e.kind === "toolCall") {
      const id = p.id as string | undefined;
      const name = (p.function?.name ?? p.tool_name) as string | undefined;
      if (id) {
        active.push({ id, name: name ?? "" });
        names.set(id, name ?? "");
      }
      meta.set(i, { nestLevel: 0, linkedToolCallId: id, linkedToolName: name });
    } else if (e.kind === "toolCallPart") {
      const top = peek();
      meta.set(i, { nestLevel: top ? 1 : 0, linkedToolCallId: top?.id, linkedToolName: top?.name });
    } else if (e.kind === "toolResult") {
      const tcId = p.tool_call_id as string | undefined;
      const name = tcId ? names.get(tcId) : undefined;
      if (tcId) {
        const idx = active.findIndex((t) => t.id === tcId);
        if (idx !== -1) active.splice(idx, 1);
      }
      meta.set(i, { nestLevel: 0, linkedToolCallId: tcId, linkedToolName: name });
    } else {
      meta.set(i, { nestLevel: 0 });
    }
  });
  return meta;
}
