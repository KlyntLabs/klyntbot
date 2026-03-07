import type { VimMode } from "./vim/VimState";

interface VimStatusLineProps {
  mode: VimMode;
}

const MODE_LABELS: Record<VimMode, string> = {
  normal: "-- NORMAL --",
  insert: "-- INSERT --",
  visual: "-- VISUAL --",
  "visual-line": "-- VISUAL LINE --",
};

export function VimStatusLine({ mode }: VimStatusLineProps) {
  return (
    <span className="font-mono text-xs text-secondary tracking-wide select-none">
      {MODE_LABELS[mode]}
    </span>
  );
}
