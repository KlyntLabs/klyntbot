export type LauncherMode = "dashboard" | "search" | "detail" | "chat";

export interface LauncherItem {
  id: string;
  title: string;
  subtitle?: string;
  icon?: string;
  kind: LauncherItemKind;
  score: number;
}

export type LauncherItemKind =
  | { type: "application"; path: string; running: boolean }
  | { type: "task"; taskId: string; status: string }
  | { type: "note"; noteId: string; preview: string }
  | { type: "clipboardEntry"; entryId: number; contentType: "text" | "image" | "file" }
  | { type: "systemCommand"; action: string }
  | { type: "script"; path: string; name: string }
  | { type: "calculator"; expression: string; result: number }
  | { type: "calendar"; eventId: string; startsAt: string }
  | { type: "aiChat"; query: string }
  | {
      type: "file";
      path: string;
      kind: "file" | "folder" | "image" | "document" | "code" | "archive";
    }
  | { type: "contentMatch"; path: string; line: number; preview: string }
  | {
      type: "contact";
      name: string;
      email: string | null;
      phone: string | null;
    }
  | { type: "systemPref"; paneId: string }
  | { type: "runningApp"; pid: number; path: string }
  | { type: "bookmark"; url: string; browser: string }
  | { type: "browserHistory"; url: string; visitedAt: string }
  | { type: "brewPackage"; name: string; isCask: boolean }
  | { type: "sshHost"; host: string; user: string | null }
  | { type: "gitRepo"; path: string };

export interface DashboardData {
  focus: FocusDashboard | null;
  calendar: CalendarDashboard[];
  tasks: TaskDashboard[];
  productivity: ProductivityDashboard;
}

export interface FocusDashboard {
  taskName: string | null;
  elapsedSecs: number;
  targetSecs: number | null;
  sessionId: string;
}

export interface CalendarDashboard {
  eventId: string;
  title: string;
  startsAt: string;
  endsAt: string;
  minutesUntil: number;
}

export interface TaskDashboard {
  id: string;
  title: string;
  status: string;
  projectName: string | null;
}

export interface ProductivityDashboard {
  totalMinutes: number;
  topCategory: string;
  topCategoryPct: number;
  score: number;
}
