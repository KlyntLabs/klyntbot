import type { VimMode } from "./vim";

interface VimStatusLineProps {
  mode: VimMode;
}

const MODE_LABELS: Record<VimMode, string> = {
  normal: "-- NORMAL --",
  insert: "-- INSERT --",
  visual: "-- VISUAL --",
  "visual-line": "-- VISUAL LINE --",
  replace: "-- REPLACE --",
};

export function VimStatusLine({ mode }: VimStatusLineProps) {
  return (
    <span className="font-mono text-ui-sm text-fg-secondary tracking-wide select-none">
      {MODE_LABELS[mode]}
    </span>
  );
}
