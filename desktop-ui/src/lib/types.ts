export interface Task {
  id: string;
  title: string;
  completed: boolean;
  priority: string | null;
  status: string;
  dueDate: string | null;
  tags: string[];
  projectId: string | null;
  areaId: string;
  objectiveId?: string;
  description?: string;
}

export interface TodayTask {
  id: string;
  title: string;
  priority: string | null;
  status: string;
  completed: boolean;
  isOverdue: boolean;
  isDueToday: boolean;
  dueDisplay: string | null;
}

export interface Project {
  id: string;
  name: string;
  color: string;
  areaId: string;
  taskCount: number;
  completedCount: number;
  objectiveIds?: string[];
}

export interface Objective {
  id: string;
  title: string;
  progress: number;
  projectId: string;
  keyResults?: KeyResult[];
}

export interface KeyResult {
  id: string;
  title: string;
  progress: number;
  current: number;
  target: number;
  unit: string;
}

export interface CalendarEvent {
  id: string;
  title: string;
  startAt: string;
  endAt: string;
  color: string;
}

export interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp?: string;
}

export interface ChatThread {
  sessionKey: string;
  title: string;
  messageCount: number;
  updatedAt: string;
  projectId: string | null;
}

export interface Area {
  id: string;
  name: string;
  color: string;
  icon: string | null;
  projectCount: number;
  taskCount: number;
}

export interface AgentStatus {
  status: string;
  activeTaskCount: number;
  focusTask: Task | null;
}

export interface LauncherItem {
  id: string;
  title: string;
  subtitle: string;
  icon: string;
  shortcut: string;
}

export type Tab = 'All' | string;
export type SidebarItem = 'Chat' | 'Tasks' | 'OKR' | 'Calendar' | 'Settings';
export type ViewMode = 'table' | 'board' | 'tree';
