import { StatusIcon, type Status } from "./status-icons";

export function renderStatusIcon(status: Status, className?: string) {
  return <StatusIcon status={status} className={className} />;
}
