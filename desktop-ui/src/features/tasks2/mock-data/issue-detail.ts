import type { MockArea } from "./areas";
import { areas } from "./areas";
import type { LabelInterface } from "./labels";
import { labels } from "./labels";
import type { Priority } from "./priorities";
import { priorities } from "./priorities";
import type { Project } from "./projects";
import { projects } from "./projects";
import type { Status } from "./status";
import { status } from "./status";

// --- Exported types ---

export type TaskState = "new" | "focused" | "has-history" | "completed";
export type ActorType = "user" | "agent" | "system";
export type FlowState = "active" | "building" | "lost";
export type FocusMode = "deep-work" | "focus" | "pomodoro";
export type EnergyLevel = "low" | "medium" | "high" | "deep";
export type TaskType = "manual" | "agentic" | "hybrid";
export type SuggestionStatus = "pending" | "applied" | "dismissed";

export interface MockDetailTask {
  id: string;
  identifier: string;
  title: string;
  description: string; // HTML for TipTap
  status: Status;
  priority: Priority;
  labels: LabelInterface[];
  project: Project | null;
  area: MockArea;
  tags: string[];
  dueDate: string | null; // ISO
  energyLevel: EnergyLevel | null;
  taskType: TaskType;
  estimatedMinutes: number | null;
  actualMinutes: number | null;
  totalTrackedSecs: number;
  focusedAt: string | null; // ISO timestamp
  acceptanceCriteria: string | null;
  completed: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface MockActivityEntry {
  id: string;
  actorType: ActorType;
  actorName: string;
  action: string;
  detail: string | null;
  createdAt: string; // ISO
}

export interface MockSuggestion {
  id: string;
  title: string;
  description: string;
  confidence: number; // 0-1
  status: SuggestionStatus;
}

export interface MockFocusSession {
  focusMode: FocusMode;
  qualityScore: number; // 0-1
  distractionCount: number;
  flowState: FlowState;
  qualityHistory: number[]; // 5-min buckets
}

export interface MockSubIssue {
  id: string;
  identifier: string;
  title: string;
  status: Status;
  priority: Priority;
  completed: boolean;
}

export interface MockTaskMemory {
  lastSessionSummary: string;
  continuityNote: string;
  relatedFacts: string[];
}

// --- Helpers (same pattern as issues.ts) ---

function s(id: string): Status {
  const found = status.find((item) => item.id === id);
  if (!found) throw new Error(`Unknown status id: "${id}"`);
  return found;
}

function p(id: string): Priority {
  const found = priorities.find((item) => item.id === id);
  if (!found) throw new Error(`Unknown priority id: "${id}"`);
  return found;
}

function pr(id: string): Project {
  const found = projects.find((item) => item.id === id);
  if (!found) throw new Error(`Unknown project id: "${id}"`);
  return found;
}

function a(id: string): MockArea {
  const found = areas.find((item) => item.id === id);
  if (!found) throw new Error(`Unknown area id: "${id}"`);
  return found;
}

const l = (...ids: string[]): LabelInterface[] => labels.filter((item) => ids.includes(item.id));

// 25 minutes ago from "now" (2026-03-13T12:00:00Z as reference)
const FOCUSED_AT = "2026-03-13T11:35:00Z";

// --- Mock data exports ---

export const mockDetailTask: MockDetailTask = {
  id: "detail-1",
  identifier: "LNUI-501",
  title: "Implement real-time collaboration cursor sync",
  description: `<h2>Overview</h2>
<p>Enable multiple users to see each other's cursors and selections in real time when collaborating on the same document or issue. This requires a WebSocket-based presence layer and a CRDT-friendly state model.</p>
<h3>Background</h3>
<p>Currently, concurrent edits lead to conflicts and lost changes. Users expect Google Docs-style live presence indicators.</p>`,
  status: s("in-progress"),
  priority: p("high"),
  labels: l("feature", "ui", "performance"),
  project: pr("1"),
  area: a("area-work"),
  tags: ["collaboration", "websocket", "crdt"],
  dueDate: "2026-03-28T00:00:00Z",
  energyLevel: "deep",
  taskType: "hybrid",
  estimatedMinutes: 240,
  actualMinutes: null,
  totalTrackedSecs: 5400, // 90 minutes tracked so far
  focusedAt: FOCUSED_AT,
  acceptanceCriteria: `- [ ] Cursor positions broadcast within 100 ms
- [ ] Presence avatars shown in toolbar
- [ ] Graceful disconnect handling
- [ ] Works across 3+ concurrent users in load test`,
  completed: false,
  createdAt: "2026-03-01T09:00:00Z",
  updatedAt: "2026-03-13T11:35:00Z",
};

export const mockActivityEntries: MockActivityEntry[] = [
  {
    id: "act-1",
    actorType: "user",
    actorName: "Alex Chen",
    action: "created issue",
    detail: null,
    createdAt: "2026-03-01T09:00:00Z",
  },
  {
    id: "act-2",
    actorType: "system",
    actorName: "System",
    action: "assigned to project",
    detail: "Core Platform",
    createdAt: "2026-03-01T09:01:00Z",
  },
  {
    id: "act-3",
    actorType: "user",
    actorName: "Alex Chen",
    action: "changed priority",
    detail: "Medium → High",
    createdAt: "2026-03-03T14:22:00Z",
  },
  {
    id: "act-4",
    actorType: "agent",
    actorName: "Klyntbot",
    action: "added acceptance criteria",
    detail: "Generated 4 criteria from task description",
    createdAt: "2026-03-05T10:15:00Z",
  },
  {
    id: "act-5",
    actorType: "user",
    actorName: "Sam Rivera",
    action: "added comment",
    detail: "We should look at Yjs as the CRDT library — it has good React bindings.",
    createdAt: "2026-03-07T16:44:00Z",
  },
  {
    id: "act-6",
    actorType: "system",
    actorName: "System",
    action: "status changed",
    detail: "Todo → In Progress",
    createdAt: "2026-03-10T08:30:00Z",
  },
  {
    id: "act-7",
    actorType: "agent",
    actorName: "Klyntbot",
    action: "linked related issues",
    detail: "Detected dependency on LNUI-488 (WebSocket connection handler)",
    createdAt: "2026-03-11T11:00:00Z",
  },
  {
    id: "act-8",
    actorType: "user",
    actorName: "Alex Chen",
    action: "started focus session",
    detail: "Deep work — 4 h block",
    createdAt: FOCUSED_AT,
  },
];

export const mockSuggestions: MockSuggestion[] = [
  {
    id: "sug-1",
    title: "Break into two sub-tasks",
    description:
      "The presence layer (WebSocket broadcast) and the UI layer (cursor rendering) are independently testable. Splitting them would reduce review scope and allow parallel work.",
    confidence: 0.87,
    status: "pending",
  },
  {
    id: "sug-2",
    title: "Add Yjs integration spike",
    description:
      "Sam Rivera mentioned Yjs in comments. A 2-hour spike to evaluate Yjs vs Automerge would de-risk the CRDT choice before full implementation.",
    confidence: 0.73,
    status: "pending",
  },
  {
    id: "sug-3",
    title: "Reuse WebSocket infrastructure from LNUI-201",
    description:
      "The memory-leak fix in LNUI-201 introduced a cleaned-up connection manager. Extending it rather than writing a new layer could save ~60 min.",
    confidence: 0.61,
    status: "pending",
  },
];

export const mockFocusSession: MockFocusSession = {
  focusMode: "deep-work",
  qualityScore: 0.78,
  distractionCount: 2,
  flowState: "active",
  qualityHistory: [0.5, 0.65, 0.72, 0.8, 0.78],
};

export const mockSubIssues: MockSubIssue[] = [
  {
    id: "sub-1",
    identifier: "LNUI-501a",
    title: "Design WebSocket presence protocol",
    status: s("completed"),
    priority: p("high"),
    completed: true,
  },
  {
    id: "sub-2",
    identifier: "LNUI-501b",
    title: "Implement server-side cursor broadcast",
    status: s("in-progress"),
    priority: p("high"),
    completed: false,
  },
  {
    id: "sub-3",
    identifier: "LNUI-501c",
    title: "Render remote cursors in editor UI",
    status: s("to-do"),
    priority: p("medium"),
    completed: false,
  },
  {
    id: "sub-4",
    identifier: "LNUI-501d",
    title: "Load-test with 10 concurrent users",
    status: s("backlog"),
    priority: p("medium"),
    completed: false,
  },
];

export const mockTaskMemory: MockTaskMemory = {
  lastSessionSummary:
    "Last session (2026-03-10): researched CRDT options, prototyped a minimal Yjs binding in a sandbox branch. WebSocket message format drafted in docs/presence-protocol.md.",
  continuityNote:
    "Next step is to wire the Yjs document into the existing EditorProvider and confirm that awareness state propagates correctly before moving to the server broadcast layer.",
  relatedFacts: [
    "LNUI-201 introduced a cleaned-up WebSocket connection manager in src/lib/ws.ts — reuse it.",
    "Team agreed in Slack (2026-03-06) to target < 100 ms cursor latency as the acceptance threshold.",
  ],
};
