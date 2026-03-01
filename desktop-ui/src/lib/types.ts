export interface Task {
  id: string;
  title: string;
  completed: boolean;
  priority: 'P1' | 'P2' | 'P3' | 'P4';
  status: 'Todo' | 'Doing' | 'Done';
  dueDate: string;
  tags: string[];
  project: string;
  area: 'All' | 'Work' | 'Personal';
  objectiveId?: string;
}

export interface Project {
  id: string;
  name: string;
  color: string;
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
  time: string;
  color: string;
}

export interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
}

export interface LauncherItem {
  id: string;
  title: string;
  subtitle: string;
  icon: string;
  shortcut: string;
}

export type Tab = 'All' | 'Work' | 'Personal';
export type SidebarItem = 'Chat' | 'Tasks' | 'OKR' | 'Calendar' | 'Settings';
export type ViewMode = 'table' | 'board' | 'tree';
