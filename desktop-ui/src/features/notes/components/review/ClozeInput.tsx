import { useRef, useState } from "react";

interface ClozeInputProps {
  /** Text with {{c1::hidden}} markers */
  clozeText: string;
  onSubmit: (filledText: string) => void;
}

interface Segment {
  type: "text" | "blank";
  content: string;
  index: number;
}

function parseCloze(text: string): Segment[] {
  const segments: Segment[] = [];
  // Match {{cN::...}} patterns
  const pattern = /\{\{c\d+::([^}]*)\}\}/g;
  let lastIndex = 0;
  let blankIndex = 0;
  let match: RegExpExecArray | null;

  // biome-ignore lint/suspicious/noAssignInExpressions: standard regex loop pattern
  while ((match = pattern.exec(text)) !== null) {
    if (match.index > lastIndex) {
      segments.push({ type: "text", content: text.slice(lastIndex, match.index), index: -1 });
    }
    segments.push({ type: "blank", content: match[1] ?? "", index: blankIndex });
    blankIndex += 1;
    lastIndex = match.index + match[0].length;
  }

  if (lastIndex < text.length) {
    segments.push({ type: "text", content: text.slice(lastIndex), index: -1 });
  }

  // If no cloze markers found, treat entire text as one blank
  if (blankIndex === 0) {
    return [{ type: "blank", content: "", index: 0 }];
  }

  return segments;
}

export function ClozeInput({ clozeText, onSubmit }: ClozeInputProps) {
  const segments = useState<Segment[]>(() => parseCloze(clozeText))[0];
  const blankCount = segments.filter((s) => s.type === "blank").length;

  const answersRef = useRef<string[]>(Array(blankCount).fill(""));
  const [, forceUpdate] = useState(0);

  const handleChange = (idx: number, value: string) => {
    answersRef.current[idx] = value;
    forceUpdate((n) => n + 1);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>, idx: number) => {
    if (e.key === "Enter") {
      e.preventDefault();
      // Move focus to next blank, or submit if last
      const inputs = document.querySelectorAll<HTMLInputElement>("[data-cloze-input]");
      const nextInput = inputs[idx + 1];
      if (nextInput) {
        nextInput.focus();
      } else {
        handleSubmit();
      }
    }
  };

  const handleSubmit = () => {
    const filled = answersRef.current.join(" | ").trim();
    if (filled) {
      onSubmit(filled);
    }
  };

  return (
    <div className="flex flex-col gap-2">
      <div className="rounded-lg bg-white/[0.04] border border-separator px-3 py-2 text-ui-sm text-fg leading-relaxed">
        {segments.map((seg) => {
          if (seg.type === "text") {
            return <span key={`text-${seg.content.slice(0, 20)}`}>{seg.content}</span>;
          }
          return (
            <input
              key={`blank-${seg.index}`}
              data-cloze-input
              type="text"
              value={answersRef.current[seg.index] ?? ""}
              onChange={(e) => handleChange(seg.index, e.target.value)}
              onKeyDown={(e) => handleKeyDown(e, seg.index)}
              placeholder="…"
              className="inline-block mx-0.5 px-1.5 py-0 min-w-[80px] max-w-[200px] bg-white/[0.06] border-b border-brand/60 text-ui-sm text-fg focus:outline-none focus:border-fg-secondary/40 rounded-sm placeholder:text-fg-dim"
              style={{
                width: `${Math.max(80, (answersRef.current[seg.index]?.length ?? 0) * 8 + 32)}px`,
              }}
            />
          );
        })}
      </div>
      <div className="flex items-center justify-between">
        <span className="text-[9px] text-fg-dim">Enter to advance fields</span>
        <button
          type="button"
          onClick={handleSubmit}
          disabled={answersRef.current.every((a) => !a.trim())}
          className="text-ui-xs px-3 py-1 rounded-md bg-brand/20 text-brand hover:bg-brand/30 disabled:opacity-40 disabled:cursor-not-allowed"
        >
          Submit
        </button>
      </div>
    </div>
  );
}
