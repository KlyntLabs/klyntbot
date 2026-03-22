// ── Active Recall / Flashcard Review ───────────────────────

export type AnswerMode =
  | "typed"
  | "self_grade"
  | "multiple_choice"
  | "cloze_fill"
  | "voice"
  | "auto";

export interface GradeResult {
  score: number | null;
  suggestedRating: string;
  gradingMethod: string;
  explanation: string | null;
  diffHighlights: Array<{ text: string; status: "match" | "missing" | "extra" | "partial" }>;
  expectedAnswer: string;
  coachingNudge: string | null;
  socraticSuggestion: string | null;
  keyConceptsPresent: string[];
  keyConceptsMissing: string[];
}

export interface SessionStats {
  cardsReviewed: number;
  totalScore: number;
  modeUsage: Record<string, { count: number; totalScore: number }>;
  weakCards: Array<{ front: string; score: number }>;
  propagationCount: number;
  startTime: number;
}

// ── Notes ──────────────────────────────────────────────────

export interface Note {
  id: string;
  notebookId: string | null;
  title: string;
  body: string;
  bodyHtml: string | null;
  pinned: boolean;
  archived: boolean;
  icon: string | null;
  color: string | null;
  tags: string[];
  splitContent: string | null;
  splitMode: string | null;
  perspectiveConfig: string | null;
  createdAt: string;
  updatedAt: string;
}

// ── Note Versions ──────────────────────────────────────────

export interface NoteVersion {
  id: string;
  noteId: string;
  body: string;
  createdAt: string;
}

// ── Inbox Items ───────────────────────────────────────────

export interface InboxItem {
  id: string;
  content: string;
  status: string;
  createdAt: string;
}

// ── Note Links ─────────────────────────────────────────────

export interface NoteLink {
  sourceId: string;
  targetId: string;
}

// ── Notebooks ──────────────────────────────────────────────

export interface Notebook {
  id: string;
  parentId: string | null;
  title: string;
  icon: string | null;
  color: string | null;
  sortOrder: number;
  noteCount: number;
}

// ── Note Mutation Parameters ───────────────────────────────

export interface NoteCreateParams {
  title: string;
  notebookId?: string;
  body?: string;
  tags?: string[];
}

export interface NoteUpdateParams {
  id: string;
  title?: string;
  body?: string;
  bodyHtml?: string;
  pinned?: boolean;
  notebookId?: string | null;
  tags?: string[];
  icon?: string | null;
  color?: string | null;
  splitContent?: string | null;
  splitMode?: string | null;
  perspectiveConfig?: string | null;
}

// ── Notebook Mutation Parameters ───────────────────────────

export interface NotebookCreateParams {
  title: string;
  parentId?: string;
  icon?: string;
}

// ── Workspace Config ──────────────────────────────────────

export interface WorkspaceFile {
  name: string;
  description: string;
  exists: boolean;
}

export interface WorkspaceFileContent {
  name: string;
  content: string;
}

// ── Agent Profiles ──────────────────────────────────────────

export interface AgentProfileSummary {
  name: string;
  description: string;
  isBuiltin: boolean;
  hasOverride: boolean;
  files: AgentFileSummary[];
}

export interface AgentFileSummary {
  filename: string;
  displayName: string;
  description: string;
  isBuiltin: boolean;
  hasOverride: boolean;
}

export interface AgentFileContent {
  agentName: string;
  filename: string;
  content: string;
  isBuiltin: boolean;
}
