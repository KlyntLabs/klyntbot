import { type Status, StatusIcon } from "./status-icons";

export function renderStatusIcon(status: Status, className?: string) {
  return <StatusIcon status={status} className={className} />;
}
