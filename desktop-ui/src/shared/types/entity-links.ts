// ── Entity Link Types ────────────────────────────────────────

export interface EntityLink {
  id: string;
  sourceKind: string;
  sourceId: string;
  targetKind: string;
  targetId: string;
  linkType: string;
  metadata?: Record<string, unknown>;
  createdAt: string;
}

export interface LinkedEntities {
  tasks: ActionSummary[];
  notes: NoteSummary[];
  conversations: SessionSummary[];
  sources: ProjectSource[];
  objectives: ObjectiveSummary[];
  keyResults: KeyResultSummary[];
}

export interface ActionSummary {
  id: string;
  title: string;
  status: string;
  priority?: string;
}

export interface NoteSummary {
  id: string;
  title: string;
  updatedAt: string;
}

export interface SessionSummary {
  key: string;
  title?: string;
  conversationType?: string;
  updatedAt: string;
}

export interface ObjectiveSummary {
  id: string;
  title: string;
  progress: number;
  status: string;
}

export interface KeyResultSummary {
  id: string;
  title: string;
  progress: number;
}

// ── Project Source Types ─────────────────────────────────────

export interface ProjectSource {
  id: string;
  projectId: string;
  sourceType: string;
  title: string;
  content?: string;
  url?: string;
  filePath?: string;
  metadata?: Record<string, unknown>;
  tags: string[];
  createdAt: string;
  updatedAt: string;
}

export interface ProjectSourceCreateParams {
  projectId: string;
  sourceType: string;
  title: string;
  content?: string;
  url?: string;
  filePath?: string;
  metadata?: Record<string, unknown>;
  tags?: string[];
}

export interface EntityLinkCreateParams {
  sourceKind: string;
  sourceId: string;
  targetKind: string;
  targetId: string;
  linkType?: string;
  metadata?: Record<string, unknown>;
}
