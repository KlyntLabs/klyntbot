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
    <div className="tool-row__tail" role="log" aria-live="polite">
      {lastLines.map((line, index) => (
        <div
          key={`tail-${index}-${line.slice(0, 16)}`}
          className={`tool-row__tail-line${index < lastLines.length - 1 ? " tool-row__tail-line--dim" : ""}`}
        >
          {line || " "}
        </div>
      ))}
    </div>
  );
});
