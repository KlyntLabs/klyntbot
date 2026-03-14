import type React from "react";

export interface Status {
  id: string;
  name: string;
  color: string;
  icon: React.FC;
}

export const BacklogIcon: React.FC = () => (
  <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
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
  <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
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
  <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
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
  <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
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
  <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
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
  <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
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

export const status: Status[] = [
  { id: "in-progress", name: "In Progress", color: "#facc15", icon: InProgressIcon },
  { id: "technical-review", name: "Technical Review", color: "#22c55e", icon: TechnicalReviewIcon },
  { id: "completed", name: "Completed", color: "#8b5cf6", icon: CompletedIcon },
  { id: "paused", name: "Paused", color: "#0ea5e9", icon: PausedIcon },
  { id: "to-do", name: "Todo", color: "#f97316", icon: ToDoIcon },
  { id: "backlog", name: "Backlog", color: "#ec4899", icon: BacklogIcon },
];

const statusById: Record<string, Status> = Object.fromEntries(status.map((s) => [s.id, s]));

export const StatusIcon: React.FC<{ statusId: string }> = ({ statusId }) => {
  const currentStatus = statusById[statusId];
  if (!currentStatus) return null;
  const IconComponent = currentStatus.icon;
  return <IconComponent />;
};
