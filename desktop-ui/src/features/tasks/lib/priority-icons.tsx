import type React from "react";

export interface Priority {
  id: string;
  name: string;
  icon: React.FC<React.SVGProps<SVGSVGElement>>;
}

export const NoPriorityIcon: React.FC<React.SVGProps<SVGSVGElement>> = (props) => (
  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" {...props}>
    <rect x="1" y="7" width="3" height="2" rx="1" fill="currentColor" opacity="0.4" />
    <rect x="6.5" y="7" width="3" height="2" rx="1" fill="currentColor" opacity="0.4" />
    <rect x="12" y="7" width="3" height="2" rx="1" fill="currentColor" opacity="0.4" />
  </svg>
);

export const UrgentPriorityIcon: React.FC<React.SVGProps<SVGSVGElement>> = (props) => (
  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" {...props}>
    <rect x="2" y="2" width="12" height="12" rx="2" fill="var(--destructive)" />
    <path d="M8 4.5V9" stroke="white" strokeWidth="1.5" strokeLinecap="round" />
    <circle cx="8" cy="11" r="0.75" fill="white" />
  </svg>
);

export const HighPriorityIcon: React.FC<React.SVGProps<SVGSVGElement>> = (props) => (
  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" {...props}>
    <rect x="1" y="8" width="3" height="6" rx="1" fill="currentColor" />
    <rect x="6" y="5" width="3" height="9" rx="1" fill="currentColor" />
    <rect x="11" y="2" width="3" height="12" rx="1" fill="currentColor" />
  </svg>
);

export const MediumPriorityIcon: React.FC<React.SVGProps<SVGSVGElement>> = (props) => (
  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" {...props}>
    <rect x="1" y="8" width="3" height="6" rx="1" fill="currentColor" />
    <rect x="6" y="5" width="3" height="9" rx="1" fill="currentColor" />
    <rect x="11" y="2" width="3" height="12" rx="1" fill="currentColor" opacity="0.3" />
  </svg>
);

export const LowPriorityIcon: React.FC<React.SVGProps<SVGSVGElement>> = (props) => (
  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" {...props}>
    <rect x="1" y="8" width="3" height="6" rx="1" fill="currentColor" />
    <rect x="6" y="5" width="3" height="9" rx="1" fill="currentColor" opacity="0.3" />
    <rect x="11" y="2" width="3" height="12" rx="1" fill="currentColor" opacity="0.3" />
  </svg>
);

export const priorities: Priority[] = [
  { id: "no-priority", name: "No priority", icon: NoPriorityIcon },
  { id: "urgent", name: "Urgent", icon: UrgentPriorityIcon },
  { id: "high", name: "High", icon: HighPriorityIcon },
  { id: "medium", name: "Medium", icon: MediumPriorityIcon },
  { id: "low", name: "Low", icon: LowPriorityIcon },
];
