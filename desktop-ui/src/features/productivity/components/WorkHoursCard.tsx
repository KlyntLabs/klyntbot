import { formatLongDuration } from "@shared/lib/dates";
import { useState } from "react";

interface WorkHoursCardProps {
  totalActiveSecs: number;
  workDayHours?: number;
}

const SEGMENTS = [
  { from: 0, to: 50, color: "var(--text-muted)", label: "Getting started", range: "0–50%" },
  { from: 50, to: 80, color: "var(--brand)", label: "Progressing", range: "50–80%" },
  { from: 80, to: 100, color: "var(--success)", label: "On target", range: "80–100%" },
  { from: 100, to: 120, color: "var(--info)", label: "Slight overtime", range: "100–120%" },
  { from: 120, to: 150, color: "var(--destructive)", label: "Heavy overtime", range: "120%+" },
];

const BAR_MAX = 150;

function getStatusColor(rawPct: number): string {
  if (rawPct < 50) return "var(--text-muted)";
  if (rawPct < 80) return "var(--brand)";
  if (rawPct <= 100) return "var(--success)";
  if (rawPct <= 120) return "var(--info)";
  return "var(--destructive)";
}

function getStatusMessage(rawPct: number, remaining: number, overtime: number): string {
  if (rawPct < 100) return `${formatLongDuration(remaining)} remaining`;
  if (rawPct <= 110) return "Target reached";
  return `${formatLongDuration(overtime)} overtime`;
}

function SegmentedBar({ clampedPct, targetSecs }: { clampedPct: number; targetSecs: number }) {
  const [hovered, setHovered] = useState<number | null>(null);

  // Compute cumulative left offset for tooltip positioning
  const visibleSegments = SEGMENTS.map((seg) => {
    const segRange = seg.to - seg.from;
    const filled = Math.max(Math.min(clampedPct - seg.from, segRange), 0);
    const widthPct = (filled / BAR_MAX) * 100;
    return { ...seg, filled, widthPct };
  }).filter((s) => s.filled > 0);

  let cumLeft = 0;
  const positioned = visibleSegments.map((s) => {
    const left = cumLeft;
    cumLeft += s.widthPct;
    return { ...s, leftPct: left };
  });

  return (
    <div className="relative">
      <div className="h-2 rounded-full bg-white/[0.08] overflow-hidden flex">
        {positioned.map((seg, i) => (
          <div
            key={seg.from}
            className="h-full transition-[width] duration-700 first:rounded-l-full last:rounded-r-full"
            style={{
              width: `${seg.widthPct}%`,
              background: seg.color,
              opacity: hovered !== null && hovered !== i ? 0.4 : 1,
              transition: "width 0.7s, opacity 0.15s",
            }}
            onMouseEnter={() => setHovered(i)}
            onMouseLeave={() => setHovered(null)}
          />
        ))}
      </div>

      {/* 100% target marker */}
      <div
        className="absolute top-0 h-2 w-px"
        style={{
          left: `${(100 / BAR_MAX) * 100}%`,
          background: "rgba(255,255,255,0.25)",
        }}
      />

      {/* Tooltip */}
      {hovered !== null && positioned[hovered] && (
        <div
          className="absolute -top-9 z-10 px-2.5 py-1 rounded-lg text-[10px] font-light text-primary whitespace-nowrap pointer-events-none"
          style={{
            left: `${positioned[hovered].leftPct + positioned[hovered].widthPct / 2}%`,
            transform: "translateX(-50%)",
            background: "var(--surface-floating)",
            border: "1px solid var(--border)",
            boxShadow: "var(--shadow-tooltip)",
          }}
        >
          <span className="font-medium">{positioned[hovered].label}</span>
          <span className="text-dim"> · {positioned[hovered].range}</span>
          <span className="text-dim">
            {" "}
            · {formatLongDuration((positioned[hovered].filled / 100) * targetSecs)}
          </span>
        </div>
      )}
    </div>
  );
}

export function WorkHoursCard({ totalActiveSecs, workDayHours = 8 }: WorkHoursCardProps) {
  const targetSecs = workDayHours * 3600;
  const rawPct = (totalActiveSecs / targetSecs) * 100;
  const clampedPct = Math.min(rawPct, BAR_MAX);
  const remaining = Math.max(targetSecs - totalActiveSecs, 0);
  const overtime = Math.max(totalActiveSecs - targetSecs, 0);
  const statusColor = getStatusColor(rawPct);

  return (
    <div className="glass-card p-4 flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-[13px] font-medium text-secondary">Work Hours</h2>
        <span className="text-[10px] font-light text-dim">of {workDayHours}h target</span>
      </div>

      {/* Hero number */}
      <div className="flex items-baseline gap-2">
        <span className="text-[28px] font-light text-primary tabular-nums leading-none">
          {formatLongDuration(totalActiveSecs)}
        </span>
        <span className="text-[13px] font-medium tabular-nums" style={{ color: statusColor }}>
          {Math.round(rawPct)}%
        </span>
      </div>

      {/* Multi-segment progress bar */}
      <SegmentedBar clampedPct={clampedPct} targetSecs={targetSecs} />

      {/* Status */}
      <span className="text-[10px] font-light text-dim">
        {getStatusMessage(rawPct, remaining, overtime)}
      </span>
    </div>
  );
}
