import type React from "react";
import { StatusIcon } from "./status-icons";

export function renderStatusIcon(statusId: string): React.ReactElement | null {
  return <StatusIcon statusId={statusId} />;
}
