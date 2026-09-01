import type { ConversationItem } from "@/types";
import { type ToolFamily, toolRowDescriptor } from "./messageRenderUtils";

export type BurstGroup = {
  id: string;
  kind: "burst";
  family: ToolFamily;
  name: string;
  items: Array<Extract<ConversationItem, { kind: "tool" }>>;
};

export type GroupedItem = ConversationItem | BurstGroup;

const MIN_BURST_SIZE = 3;

function isFailed(item: Extract<ConversationItem, { kind: "tool" }>): boolean {
  return /(fail|error)/i.test(item.status ?? "");
}

export function groupBursts(items: ConversationItem[]): GroupedItem[] {
  const out: GroupedItem[] = [];
  let i = 0;
  while (i < items.length) {
    const item = items[i];
    if (item.kind !== "tool" || isFailed(item)) {
      out.push(item);
      i += 1;
      continue;
    }
    const baseDesc = toolRowDescriptor(item);
    let j = i;
    while (j < items.length) {
      const candidate = items[j];
      if (candidate.kind !== "tool") break;
      if (isFailed(candidate)) break;
      const desc = toolRowDescriptor(candidate);
      if (desc.family !== baseDesc.family || desc.name !== baseDesc.name) break;
      j += 1;
    }
    const runLength = j - i;
    if (runLength >= MIN_BURST_SIZE) {
      const groupItems = items.slice(i, j) as Array<Extract<ConversationItem, { kind: "tool" }>>;
      out.push({
        id: `burst-${groupItems[0].id}`,
        kind: "burst",
        family: baseDesc.family,
        name: baseDesc.name,
        items: groupItems,
      });
    } else {
      for (let k = i; k < j; k += 1) out.push(items[k]);
    }
    i = j;
  }
  return out;
}
