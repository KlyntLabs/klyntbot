import type { TimelineEntry } from "@/bindings";

export interface SessionBlock {
  id: string;
  startMin: number;
  endMin: number;
}

interface Props {
  entries?: TimelineEntry[];
  pxPerMin?: number;
}

export function ActivityTrack(_props: Props) {
  return null;
}
