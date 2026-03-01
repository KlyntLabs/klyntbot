import type { Task, Project, Objective, CalendarEvent, ChatMessage, LauncherItem } from '../lib/types';

export const mockProjects: Project[] = [
  { id: '1', name: 'Product Launch', color: '#F97316', taskCount: 4, completedCount: 1, objectiveIds: ['1', '2', '3'] },
  { id: '2', name: 'Mobile Redesign', color: '#3B82F6', taskCount: 3, completedCount: 1, objectiveIds: ['4', '5'] },
  { id: '3', name: 'Marketing Campaign', color: '#10B981', taskCount: 2, completedCount: 0, objectiveIds: ['6', '7'] },
  { id: '4', name: 'API Integration', color: '#8B5CF6', taskCount: 3, completedCount: 2, objectiveIds: ['8'] },
  { id: '6', name: 'CryptoGuard', color: '#EC4899', taskCount: 3, completedCount: 0, objectiveIds: ['9', '10'] },
  { id: '5', name: 'Personal', color: '#6B7280', taskCount: 2, completedCount: 0 },
];

export const mockTasks: Task[] = [
  // Product Launch (4 tasks)
  {
    id: '1',
    title: 'Design landing page mockups',
    completed: false,
    priority: 'P1',
    status: 'Doing',
    dueDate: 'Mar 3',
    tags: ['design', 'ui'],
    project: '1',
    area: 'Work',
    objectiveId: '1',
  },
  {
    id: '2',
    title: 'Implement payment gateway',
    completed: true,
    priority: 'P1',
    status: 'Done',
    dueDate: 'Mar 2',
    tags: ['dev', 'backend'],
    project: '1',
    area: 'Work',
    objectiveId: '2',
  },
  {
    id: '3',
    title: 'Write product documentation',
    completed: false,
    priority: 'P2',
    status: 'Todo',
    dueDate: 'Mar 5',
    tags: ['docs'],
    project: '1',
    area: 'Work',
    objectiveId: '3',
  },
  {
    id: '4',
    title: 'Set up analytics tracking',
    completed: false,
    priority: 'P2',
    status: 'Todo',
    dueDate: 'Mar 6',
    tags: ['analytics', 'dev'],
    project: '1',
    area: 'Work',
    objectiveId: '1',
  },

  // Mobile Redesign (3 tasks)
  {
    id: '5',
    title: 'Create mobile navigation prototype',
    completed: true,
    priority: 'P1',
    status: 'Done',
    dueDate: 'Mar 1',
    tags: ['design', 'mobile'],
    project: '2',
    area: 'Work',
    objectiveId: '4',
  },
  {
    id: '6',
    title: 'Optimize images for mobile',
    completed: false,
    priority: 'P2',
    status: 'Doing',
    dueDate: 'Mar 4',
    tags: ['performance'],
    project: '2',
    area: 'Work',
    objectiveId: '5',
  },
  {
    id: '7',
    title: 'Test responsive breakpoints',
    completed: false,
    priority: 'P3',
    status: 'Todo',
    dueDate: 'Mar 7',
    tags: ['qa', 'mobile'],
    project: '2',
    area: 'Work',
    objectiveId: '4',
  },

  // Marketing Campaign (2 tasks)
  {
    id: '8',
    title: 'Launch social media ads',
    completed: false,
    priority: 'P1',
    status: 'Doing',
    dueDate: 'Mar 3',
    tags: ['marketing', 'ads'],
    project: '3',
    area: 'Work',
    objectiveId: '6',
  },
  {
    id: '9',
    title: 'Create email newsletter template',
    completed: false,
    priority: 'P2',
    status: 'Todo',
    dueDate: 'Mar 5',
    tags: ['marketing', 'email'],
    project: '3',
    area: 'Work',
    objectiveId: '7',
  },

  // API Integration (3 tasks)
  {
    id: '10',
    title: 'Review API documentation',
    completed: true,
    priority: 'P1',
    status: 'Done',
    dueDate: 'Mar 1',
    tags: ['dev', 'backend'],
    project: '4',
    area: 'Work',
    objectiveId: '8',
  },
  {
    id: '11',
    title: 'Build webhook handlers',
    completed: true,
    priority: 'P1',
    status: 'Done',
    dueDate: 'Mar 2',
    tags: ['dev', 'backend'],
    project: '4',
    area: 'Work',
    objectiveId: '8',
  },
  {
    id: '12',
    title: 'Write integration tests',
    completed: false,
    priority: 'P2',
    status: 'Doing',
    dueDate: 'Mar 4',
    tags: ['dev', 'testing'],
    project: '4',
    area: 'Work',
    objectiveId: '8',
  },

  // CryptoGuard (3 tasks)
  {
    id: '13',
    title: 'Implement wallet connection',
    completed: false,
    priority: 'P1',
    status: 'Doing',
    dueDate: 'Mar 4',
    tags: ['web3', 'dev'],
    project: '6',
    area: 'Work',
    objectiveId: '9',
  },
  {
    id: '14',
    title: 'Audit smart contract security',
    completed: false,
    priority: 'P1',
    status: 'Todo',
    dueDate: 'Mar 6',
    tags: ['security', 'blockchain'],
    project: '6',
    area: 'Work',
    objectiveId: '10',
  },
  {
    id: '15',
    title: 'Design transaction dashboard',
    completed: false,
    priority: 'P2',
    status: 'Todo',
    dueDate: 'Mar 7',
    tags: ['design', 'ui'],
    project: '6',
    area: 'Work',
    objectiveId: '9',
  },

  // Personal (2 tasks)
  {
    id: '16',
    title: 'Book dentist appointment',
    completed: false,
    priority: 'P4',
    status: 'Todo',
    dueDate: 'Mar 8',
    tags: ['health'],
    project: '5',
    area: 'Personal',
  },
  {
    id: '17',
    title: 'Buy groceries',
    completed: false,
    priority: 'P4',
    status: 'Todo',
    dueDate: 'Mar 3',
    tags: ['shopping'],
    project: '5',
    area: 'Personal',
  },
];

export const mockObjectives: Objective[] = [
  // Product Launch (3 objectives with Key Results)
  {
    id: '1',
    title: 'Increase User Engagement',
    progress: 68,
    projectId: '1',
    keyResults: [
      { id: '1-1', title: 'Achieve 50K monthly active users', progress: 75, current: 37500, target: 50000, unit: 'users' },
      { id: '1-2', title: 'Reduce churn rate to <5%', progress: 60, current: 6.5, target: 5, unit: '%' },
      { id: '1-3', title: 'Increase session duration by 30%', progress: 70, current: 21, target: 30, unit: 'minutes' },
    ],
  },
  {
    id: '2',
    title: 'Improve Conversion Rate',
    progress: 52,
    projectId: '1',
    keyResults: [
      { id: '2-1', title: 'Increase trial-to-paid conversion to 25%', progress: 48, current: 18, target: 25, unit: '%' },
      { id: '2-2', title: 'Generate $100K MRR', progress: 55, current: 55000, target: 100000, unit: '$' },
    ],
  },
  {
    id: '3',
    title: 'Enhance Product Quality',
    progress: 75,
    projectId: '1',
    keyResults: [
      { id: '3-1', title: 'Achieve 95% customer satisfaction score', progress: 78, current: 90, target: 95, unit: '%' },
      { id: '3-2', title: 'Reduce critical bugs to <5', progress: 70, current: 6, target: 5, unit: 'bugs' },
      { id: '3-3', title: 'Launch 3 major features', progress: 67, current: 2, target: 3, unit: 'features' },
    ],
  },

  // Mobile Redesign (2 objectives)
  { id: '4', title: 'Optimize Mobile UX', progress: 61, projectId: '2' },
  { id: '5', title: 'Reduce Load Times', progress: 43, projectId: '2' },

  // Marketing Campaign (2 objectives)
  { id: '6', title: 'Increase Brand Awareness', progress: 35, projectId: '3' },
  { id: '7', title: 'Grow Email Subscribers', progress: 58, projectId: '3' },

  // API Integration (1 objective)
  { id: '8', title: 'Complete Third-Party Integration', progress: 82, projectId: '4' },

  // CryptoGuard (2 objectives)
  { id: '9', title: 'Launch Web3 Features', progress: 47, projectId: '6' },
  { id: '10', title: 'Ensure Platform Security', progress: 29, projectId: '6' },
];

export const mockCalendarEvents: CalendarEvent[] = [
  { id: '1', title: 'Team Standup', time: '10:00 AM', color: '#F97316' },
  { id: '2', title: 'Product Review', time: '2:00 PM', color: '#3B82F6' },
  { id: '3', title: 'Design Sync', time: '4:30 PM', color: '#10B981' },
];

export const mockChatMessages: ChatMessage[] = [
  {
    id: '1',
    role: 'user',
    content: 'What are my highest priority tasks for today?',
  },
  {
    id: '2',
    role: 'assistant',
    content: 'Based on your current tasks, here are your top priorities:\n\n1. **Design landing page mockups** (P1) - Due Mar 3\n2. **Implement payment gateway** (P1) - Due Mar 2 \u2713 Completed\n3. **Review API documentation** (P2) - Due Mar 4\n\nWould you like me to help you focus on the landing page mockups first?',
  },
  {
    id: '3',
    role: 'user',
    content: 'Yes, show me similar design examples',
  },
];

export const mockLauncherItems: LauncherItem[] = [
  { id: '1', title: 'Create New Task', subtitle: 'Quick add a new task', icon: 'Plus', shortcut: '\u2318N' },
  { id: '2', title: 'Search Tasks', subtitle: 'Find tasks across all projects', icon: 'Search', shortcut: '\u2318K' },
  { id: '3', title: 'Open Chat', subtitle: 'Talk to AI assistant', icon: 'MessageSquare', shortcut: '\u2318/' },
  { id: '4', title: 'View Calendar', subtitle: 'See upcoming events', icon: 'Calendar', shortcut: '\u2318C' },
  { id: '5', title: "Today's Focus", subtitle: 'View daily priorities', icon: 'Target', shortcut: '\u2318T' },
  { id: '6', title: 'Review OKRs', subtitle: 'Check goal progress', icon: 'TrendingUp', shortcut: '\u2318O' },
  { id: '7', title: 'Settings', subtitle: 'App preferences', icon: 'Settings', shortcut: '\u2318,' },
];
