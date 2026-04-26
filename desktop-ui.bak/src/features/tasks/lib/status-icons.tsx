import type { StatusGroup } from "@shared/types/common";
import type React from "react";

export interface Status {
  id: string;
  name: string;
  color: string;
  icon: React.FC<{ className?: string }>;
  /** The raw value stored in task.status — used for backend mutations */
  backendStatus: string;
  statusGroup?: StatusGroup;
}

export const BacklogIcon: React.FC = () => (
  <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
    <circle
      cx="7"
      cy="7"
      r="6"
      fill="none"
      stroke="#bec2c8"
      strokeWidth="2"
      strokeDasharray="1.4 1.74"
      strokeDashoffset="0.65"
    />
    <circle
      className="progress"
      cx="7"
      cy="7"
      r="2"
      fill="none"
      stroke="#bec2c8"
      strokeWidth="4"
      strokeDasharray="0 100"
      strokeDashoffset="0"
      transform="rotate(-90 7 7)"
    />
  </svg>
);

export const PausedIcon: React.FC = () => (
  <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
    <circle cx="7" cy="7" r="6" fill="none" stroke="#0ea5e9" strokeWidth="2" />
    <circle
      cx="7"
      cy="7"
      r="2"
      fill="none"
      stroke="#0ea5e9"
      strokeWidth="4"
      strokeDasharray="6 100"
      strokeDashoffset="0"
      transform="rotate(-90 7 7)"
    />
  </svg>
);

export const ToDoIcon: React.FC = () => (
  <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
    <circle cx="7" cy="7" r="6" fill="none" stroke="#e2e2e2" strokeWidth="2" />
    <circle
      cx="7"
      cy="7"
      r="2"
      fill="none"
      stroke="#e2e2e2"
      strokeWidth="4"
      strokeDasharray="0 100"
      strokeDashoffset="0"
      transform="rotate(-90 7 7)"
    />
  </svg>
);

export const InProgressIcon: React.FC = () => (
  <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
    <circle cx="7" cy="7" r="6" fill="none" stroke="#facc15" strokeWidth="2" />
    <circle
      cx="7"
      cy="7"
      r="2"
      fill="none"
      stroke="#facc15"
      strokeWidth="4"
      strokeDasharray="4.2 100"
      strokeDashoffset="0"
      transform="rotate(-90 7 7)"
    />
  </svg>
);

export const TechnicalReviewIcon: React.FC = () => (
  <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
    <circle cx="7" cy="7" r="6" fill="none" stroke="#22c55e" strokeWidth="2" />
    <circle
      cx="7"
      cy="7"
      r="2"
      fill="none"
      stroke="#22c55e"
      strokeWidth="4"
      strokeDasharray="8.4 100"
      strokeDashoffset="0"
      transform="rotate(-90 7 7)"
    />
  </svg>
);

export const CompletedIcon: React.FC = () => (
  <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
    <circle cx="7" cy="7" r="6" fill="#8b5cf6" stroke="#8b5cf6" strokeWidth="2" />
    <path
      d="M4.5 7L6.5 9L9.5 5"
      stroke="white"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);

// ── Icon matching ────────────────────────────────────────

const ICON_NAME_MAP: Record<string, React.FC<{ className?: string }>> = {
  backlog: BacklogIcon,
  todo: ToDoIcon,
  "to do": ToDoIcon,
  "in progress": InProgressIcon,
  inprogress: InProgressIcon,
  "in review": TechnicalReviewIcon,
  inreview: TechnicalReviewIcon,
  "technical review": TechnicalReviewIcon,
  review: TechnicalReviewIcon,
  done: CompletedIcon,
  completed: CompletedIcon,
  complete: CompletedIcon,
  blocked: PausedIcon,
  paused: PausedIcon,
  "on hold": PausedIcon,
};

function normalizeName(name: string): string {
  return name.toLowerCase().trim();
}

/**
 * Match a status name to a known icon component.
 * Returns undefined if no match is found.
 */
export function matchIcon(name: string): React.FC<{ className?: string }> | undefined {
  return ICON_NAME_MAP[normalizeName(name)];
}

/**
 * Create a colored circle icon component for statuses without a known icon.
 */
export function makeColoredCircle(color: string): React.FC<{ className?: string }> {
  return ({ className }) => (
    <svg
      className={className}
      width="14"
      height="14"
      viewBox="0 0 14 14"
      fill="none"
      aria-hidden="true"
    >
      <circle cx="7" cy="7" r="5" stroke={color} strokeWidth="1.5" fill="none" />
      <circle cx="7" cy="7" r="2" fill={color} />
    </svg>
  );
}

// ── StatusIcon component ─────────────────────────────────

export const StatusIcon: React.FC<{ status: Status; className?: string }> = ({
  status,
  className,
}) => {
  const IconComponent = status.icon;
  return <IconComponent className={className} />;
};
