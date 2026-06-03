import { memo, useMemo } from "react";

type BashTailProps = {
  output: string;
};

const TAIL_LINES = 3;

export const BashTail = memo(function BashTail({ output }: BashTailProps) {
  const lastLines = useMemo(() => {
    if (!output) return [];
    const lines = output.split(/\r?\n/);
    const trimmed = lines.length > 0 && lines[lines.length - 1] === "" ? lines.slice(0, -1) : lines;
    return trimmed.slice(-TAIL_LINES);
  }, [output]);

  if (lastLines.length === 0) return null;

  return (
    <div
      className="flex flex-col gap-[2px] -mt-0.5 mb-1 ml-3.5 py-1.5 px-3 bg-[var(--cm-surface-command-panel)] rounded-r-[6px] font-code text-[11px] text-text-quiet leading-[1.45] overflow-hidden"
      style={{
        maxHeight: "calc(1.45em * 3 + 12px)",
        borderLeft: "2px solid var(--tool-row-bar, var(--border-subtle))",
      }}
      role="log"
      aria-live="polite"
    >
      {lastLines.map((line, index) => (
        <div
          key={`tail-${index}-${line.slice(0, 16)}`}
          className={`whitespace-pre-wrap break-words${index < lastLines.length - 1 ? " text-text-faint" : ""}`}
        >
          {line || " "}
        </div>
      ))}
    </div>
  );
});
