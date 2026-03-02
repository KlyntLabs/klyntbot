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
  status: string;
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

export type MessageSegment =
  | { type: 'text'; content: string }
  | { type: 'tool'; name: string; success: boolean; durationMs: number; result?: string };

export interface ChatMessage {
  id: string;
  role: 'user' | 'assistant' | 'interaction';
  content: string;
  timestamp?: string;
  segments?: MessageSegment[];
}

export interface ChatThread {
  sessionKey: string;
  title: string;
  messageCount: number;
  updatedAt: string;
  contextType?: string;
  entityKind?: string;
  entityId?: string;
  areaId?: string;
  areaName?: string;
  projectId?: string;
  projectName?: string;
}

export interface SessionContext {
  entityKind?: string;
  entityId?: string;
  contextType?: string;
  isEphemeral?: boolean;
}

// ── Agent Streaming Events ──────────────────────────────────────────────

export interface ContentChunkPayload {
  sessionKey: string;
  data: string;
}

export interface ToolStartPayload {
  sessionKey: string;
  name: string;
}

export interface ToolEndPayload {
  sessionKey: string;
  name: string;
  success: boolean;
  durationMs: number;
  result?: string;
}

export interface AgentDonePayload {
  sessionKey: string;
  content: string;
}

export interface AgentErrorPayload {
  sessionKey: string;
  message: string;
}

export interface ClassificationCompletePayload {
  sessionKey: string;
  strategy: string;
  confidence: number;
}

export interface ExecutionStartedPayload {
  sessionKey: string;
  engine: string;
  maxIterations: number;
}

// ── Interaction (ask_user) ─────────────────────────────────────────────

export interface ActiveInteraction {
  requestId: string;
  request: InteractionRequest;
}

export interface InteractionRequestPayload {
  sessionKey: string;
  requestId: string;
  request: InteractionRequest;
}

export interface InteractionRequest {
  title: string;
  questions: Question[];
}

export interface Question {
  id: string;
  title: string;
  text: string;
  answer_type: AnswerType;
}

export type AnswerType =
  | { type: 'single_select'; options: AnswerOption[] }
  | { type: 'multi_select'; options: AnswerOption[] }
  | { type: 'yes_no'; default?: boolean }
  | { type: 'free_text'; placeholder?: string };

export interface AnswerOption {
  value: string;
  label: string;
  description?: string;
}

export type AnswerValue =
  | { type: 'selected'; value: string }
  | { type: 'multi_selected'; values: string[] }
  | { type: 'yes_no'; answer: boolean }
  | { type: 'text'; content: string }
  | { type: 'skipped' };

export interface Answer {
  question_id: string;
  value: AnswerValue;
}

export type FormResponse =
  | { Completed: Answer[] }
  | 'Cancelled';

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

// ── Finance ─────────────────────────────────────────────────────────────

export interface FinanceAccount {
  id: string;
  name: string;
  accountType: string;
  currency: string;
  balance: number;
  institution: string | null;
  notes: string | null;
  isArchived: boolean;
}

export interface FinanceTransaction {
  id: string;
  accountId: string;
  txType: 'income' | 'expense' | 'transfer';
  amount: number;
  currency: string;
  category: string | null;
  subcategory: string | null;
  counterparty: string | null;
  notes: string | null;
  txDate: string;
  transferId: string | null;
}

export interface FinanceBudgetUsage {
  id: string;
  name: string;
  amount: number;
  currency: string;
  period: string;
  category: string | null;
  method: string;
  jarType: string | null;
  isActive: boolean;
  alertThreshold: number;
  spent: number;
}

export interface FinancePortfolio {
  id: string;
  name: string;
  description: string | null;
  currency: string;
  totalValue: number;
  totalCostBasis: number;
  holdingCount: number;
}

export interface FinanceInvestment {
  id: string;
  portfolioId: string;
  assetType: string;
  symbol: string | null;
  name: string;
  quantity: number;
  costBasis: number;
  currency: string;
  currentPrice: number | null;
  currentValue: number | null;
}

export interface FinanceGoal {
  id: string;
  name: string;
  goalType: string;
  targetAmount: number;
  currentAmount: number;
  currency: string;
  status: string;
  deadline: string | null;
  monthlyContribution: number | null;
}

export interface FinanceLiability {
  id: string;
  name: string;
  liabilityType: string;
  principal: number;
  remaining: number;
  currency: string;
  interestRate: number | null;
  monthlyPayment: number | null;
  dueDate: string | null;
}

export interface FinanceNetWorth {
  totalsByCurrency: { currency: string; accounts: number; investments: number; liabilities: number; net: number }[];
}

export type Tab = 'All' | string;
export type SidebarItem = 'Chat' | 'Tasks' | 'OKR' | 'Calendar' | 'Finance' | 'Settings';
export type ViewMode = 'table' | 'board' | 'tree';

// ── Mutation Params ─────────────────────────────────────────────────────

export interface TaskUpdateParams {
  id: string;
  title?: string;
  description?: string | null;
  priority?: number | null;
  status?: string;
  dueDate?: string | null;
  projectId?: string | null;
  areaId?: string;
  tags?: string[];
  keyResultId?: string | null;
}

export interface AreaCreateParams {
  name: string;
  color?: string;
  icon?: string;
}

export interface AreaUpdateParams {
  id: string;
  name?: string;
  color?: string;
  icon?: string | null;
}

export interface ProjectCreateParams {
  name: string;
  areaId: string;
  color?: string;
  description?: string;
  tags?: string[];
}

export interface ProjectUpdateParams {
  id: string;
  name?: string;
  areaId?: string;
  color?: string;
  description?: string | null;
  tags?: string[];
  status?: string;
}

export interface ObjectiveCreateParams {
  title: string;
  projectId: string;
  description?: string;
  priority?: number;
  dueDate?: string;
}

export interface ObjectiveUpdateParams {
  id: string;
  title?: string;
  description?: string | null;
  status?: string;
  priority?: number | null;
  dueDate?: string | null;
}

export interface KeyResultCreateParams {
  objectiveId: string;
  title: string;
  targetValue?: number;
  unit?: string;
  trackingMode?: string;
}

export interface KeyResultUpdateParams {
  id: string;
  title?: string;
  description?: string | null;
  status?: string;
  dueDate?: string | null;
}
