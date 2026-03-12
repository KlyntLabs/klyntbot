import type React from "react";
import { StatusIcon } from "../mock-data/status";

export function renderStatusIcon(statusId: string): React.ReactElement | null {
  return <StatusIcon statusId={statusId} />;
}
