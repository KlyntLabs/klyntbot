import type { TraceEvent } from "../types";

interface Props {
  event: TraceEvent;
}

export function CompactionMarkerCard({ event }: Props) {
  const begin = event.rawKind === "CompactionBegin";
  return (
    <div
      className={
        "tracing-evcard tracing-evcard--compact " +
        (begin ? "tracing-evcard--compact-begin" : "tracing-evcard--compact-end")
      }
    >
      ⎯⎯⎯ {begin ? "Compaction begin" : "Compaction end"} ⎯⎯⎯
    </div>
  );
}
