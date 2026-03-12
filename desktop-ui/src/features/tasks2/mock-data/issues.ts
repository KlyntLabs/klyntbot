import type { LabelInterface } from "./labels";
import { labels } from "./labels";
import type { Priority } from "./priorities";
import { priorities } from "./priorities";
import type { Project } from "./projects";
import { projects } from "./projects";
import type { Status } from "./status";
import { status } from "./status";
import type { User } from "./users";
import { users } from "./users";

export interface Issue {
  id: string;
  identifier: string;
  title: string;
  description: string;
  status: Status;
  assignee: User | null;
  priority: Priority;
  labels: LabelInterface[];
  createdAt: string;
  cycleId: string;
  project?: Project;
  subissues?: string[];
  rank: string;
  dueDate?: string;
}

function s(id: string): Status {
  const found = status.find((s) => s.id === id);
  if (!found) throw new Error(`Unknown status id: "${id}"`);
  return found;
}
function p(id: string): Priority {
  const found = priorities.find((p) => p.id === id);
  if (!found) throw new Error(`Unknown priority id: "${id}"`);
  return found;
}
function u(id: string): User {
  const found = users.find((u) => u.id === id);
  if (!found) throw new Error(`Unknown user id: "${id}"`);
  return found;
}
const l = (...ids: string[]): LabelInterface[] => labels.filter((l) => ids.includes(l.id));
function pr(id: string): Project {
  const found = projects.find((p) => p.id === id);
  if (!found) throw new Error(`Unknown project id: "${id}"`);
  return found;
}

export const issues: Issue[] = [
  {
    id: "1",
    identifier: "LNUI-101",
    title: "Implement drag-and-drop issue reordering",
    description:
      "Add drag-and-drop support for reordering issues within status groups using DnD Kit.",
    status: s("in-progress"),
    assignee: u("1"),
    priority: p("high"),
    labels: l("ui", "feature"),
    createdAt: "2026-01-15T10:00:00Z",
    cycleId: "cycle-1",
    project: pr("1"),
    rank: "0|hzzzzz:",
    dueDate: "2026-03-20T00:00:00Z",
  },
  {
    id: "2",
    identifier: "LNUI-102",
    title: "Design token system for consistent theming",
    description: "Create a comprehensive design token system using CSS custom properties.",
    status: s("technical-review"),
    assignee: u("2"),
    priority: p("medium"),
    labels: l("design", "ui"),
    createdAt: "2026-01-16T09:00:00Z",
    cycleId: "cycle-1",
    project: pr("2"),
    rank: "0|hzzzzy:",
    dueDate: "2026-03-15T00:00:00Z",
  },
  {
    id: "3",
    identifier: "LNUI-103",
    title: "Fix authentication token refresh on expiry",
    description:
      "Silent token refresh is failing when the access token expires during long sessions.",
    status: s("backlog"),
    assignee: u("3"),
    priority: p("urgent"),
    labels: l("bug", "security"),
    createdAt: "2026-01-17T11:00:00Z",
    cycleId: "cycle-1",
    project: pr("3"),
    rank: "0|hzzzzx:",
  },
  {
    id: "4",
    identifier: "LNUI-104",
    title: "Add keyboard shortcuts for issue actions",
    description:
      "Implement keyboard shortcuts for common issue operations like status change and priority.",
    status: s("to-do"),
    assignee: u("4"),
    priority: p("medium"),
    labels: l("feature", "accessibility"),
    createdAt: "2026-01-18T14:00:00Z",
    cycleId: "cycle-1",
    project: pr("4"),
    rank: "0|hzzzzw:",
    dueDate: "2026-04-01T00:00:00Z",
  },
  {
    id: "5",
    identifier: "LNUI-105",
    title: "Optimize API response caching strategy",
    description:
      "Implement intelligent caching for API responses to reduce redundant network calls.",
    status: s("completed"),
    assignee: u("1"),
    priority: p("high"),
    labels: l("performance"),
    createdAt: "2026-01-19T08:00:00Z",
    cycleId: "cycle-1",
    project: pr("5"),
    rank: "0|hzzzzv:",
  },
  {
    id: "6",
    identifier: "LNUI-106",
    title: "Implement full-text search across issues",
    description: "Add real-time full-text search with highlighting across all issue fields.",
    status: s("paused"),
    assignee: u("2"),
    priority: p("low"),
    labels: l("feature"),
    createdAt: "2026-01-20T15:00:00Z",
    cycleId: "cycle-1",
    project: pr("6"),
    rank: "0|hzzzzu:",
  },
  {
    id: "7",
    identifier: "LNUI-107",
    title: "Data pipeline migration to new schema",
    description: "Migrate existing data pipeline to support the new normalized schema.",
    status: s("in-progress"),
    assignee: u("3"),
    priority: p("urgent"),
    labels: l("refactor"),
    createdAt: "2026-01-21T10:00:00Z",
    cycleId: "cycle-2",
    project: pr("7"),
    rank: "0|hzzzzt:",
    dueDate: "2026-03-25T00:00:00Z",
  },
  {
    id: "8",
    identifier: "LNUI-108",
    title: "Mobile app offline mode support",
    description: "Enable offline data access and background sync for the mobile application.",
    status: s("backlog"),
    assignee: u("4"),
    priority: p("medium"),
    labels: l("feature"),
    createdAt: "2026-01-22T09:00:00Z",
    cycleId: "cycle-2",
    project: pr("8"),
    rank: "0|hzzzzts:",
  },
  {
    id: "9",
    identifier: "LNUI-109",
    title: "CLI tools auto-completion for bash/zsh",
    description: "Add shell completion scripts for all CLI commands in bash and zsh.",
    status: s("to-do"),
    assignee: u("1"),
    priority: p("low"),
    labels: l("feature"),
    createdAt: "2026-01-23T11:00:00Z",
    cycleId: "cycle-2",
    project: pr("9"),
    rank: "0|hzzzzs:",
  },
  {
    id: "10",
    identifier: "LNUI-110",
    title: "Update API documentation with OpenAPI 3.1",
    description: "Migrate all API documentation to OpenAPI 3.1 specification format.",
    status: s("technical-review"),
    assignee: u("2"),
    priority: p("medium"),
    labels: l("documentation"),
    createdAt: "2026-01-24T14:00:00Z",
    cycleId: "cycle-2",
    project: pr("10"),
    rank: "0|hzzzzr:",
    dueDate: "2026-03-18T00:00:00Z",
  },
  {
    id: "11",
    identifier: "LNUI-201",
    title: "Fix memory leak in websocket connection handler",
    description: "Websocket connections are not properly cleaned up when components unmount.",
    status: s("in-progress"),
    assignee: u("3"),
    priority: p("urgent"),
    labels: l("bug", "performance"),
    createdAt: "2026-01-25T08:00:00Z",
    cycleId: "cycle-2",
    project: pr("1"),
    rank: "0|hzzzzq:",
    dueDate: "2026-03-14T00:00:00Z",
  },
  {
    id: "12",
    identifier: "LNUI-202",
    title: "Implement role-based access control",
    description: "Add RBAC with admin, editor, and viewer roles with granular permissions.",
    status: s("backlog"),
    assignee: u("4"),
    priority: p("high"),
    labels: l("security", "feature"),
    createdAt: "2026-01-26T09:00:00Z",
    cycleId: "cycle-2",
    project: pr("3"),
    rank: "0|hzzzzp:",
  },
  {
    id: "13",
    identifier: "LNUI-203",
    title: "Add internationalization support (i18n)",
    description: "Integrate react-i18next for multi-language support starting with EN, ES, FR.",
    status: s("to-do"),
    assignee: u("1"),
    priority: p("medium"),
    labels: l("internationalization"),
    createdAt: "2026-01-27T11:00:00Z",
    cycleId: "cycle-3",
    project: pr("4"),
    rank: "0|hzzzzon:",
  },
  {
    id: "14",
    identifier: "LNUI-204",
    title: "Create component storybook documentation",
    description: "Set up Storybook and document all design system components with examples.",
    status: s("paused"),
    assignee: u("2"),
    priority: p("low"),
    labels: l("documentation", "ui"),
    createdAt: "2026-01-28T14:00:00Z",
    cycleId: "cycle-3",
    project: pr("2"),
    rank: "0|hzzzzom:",
  },
  {
    id: "15",
    identifier: "LNUI-205",
    title: "Gateway rate limiting and throttling",
    description:
      "Implement per-client rate limiting at the API gateway with configurable thresholds.",
    status: s("completed"),
    assignee: u("3"),
    priority: p("high"),
    labels: l("security", "performance"),
    createdAt: "2026-01-29T08:00:00Z",
    cycleId: "cycle-3",
    project: pr("5"),
    rank: "0|hzzzzol:",
  },
  {
    id: "16",
    identifier: "LNUI-206",
    title: "Implement semantic search with embeddings",
    description: "Integrate vector embeddings for semantic similarity search across issues.",
    status: s("technical-review"),
    assignee: u("4"),
    priority: p("medium"),
    labels: l("feature", "performance"),
    createdAt: "2026-01-30T09:00:00Z",
    cycleId: "cycle-3",
    project: pr("6"),
    rank: "0|hzzzzok:",
    dueDate: "2026-04-10T00:00:00Z",
  },
  {
    id: "17",
    identifier: "LNUI-207",
    title: "Add unit tests for data transformers",
    description: "Write comprehensive unit tests for all data transformation utilities.",
    status: s("in-progress"),
    assignee: u("1"),
    priority: p("medium"),
    labels: l("testing"),
    createdAt: "2026-02-01T10:00:00Z",
    cycleId: "cycle-3",
    project: pr("7"),
    rank: "0|hzzzzoj:",
  },
  {
    id: "18",
    identifier: "LNUI-208",
    title: "Push notification support for mobile",
    description: "Integrate Firebase Cloud Messaging for push notifications on iOS and Android.",
    status: s("backlog"),
    assignee: u("2"),
    priority: p("medium"),
    labels: l("feature"),
    createdAt: "2026-02-02T09:00:00Z",
    cycleId: "cycle-3",
    project: pr("8"),
    rank: "0|hzzzzons:",
  },
  {
    id: "19",
    identifier: "LNUI-209",
    title: "CLI plugin system for extensibility",
    description:
      "Design and implement a plugin system for the CLI to allow third-party extensions.",
    status: s("to-do"),
    assignee: u("3"),
    priority: p("low"),
    labels: l("feature", "refactor"),
    createdAt: "2026-02-03T11:00:00Z",
    cycleId: "cycle-4",
    project: pr("9"),
    rank: "0|hzzzzont:",
  },
  {
    id: "20",
    identifier: "LNUI-210",
    title: "Auto-generate changelog from commits",
    description: "Set up conventional commits parsing to auto-generate release changelogs.",
    status: s("paused"),
    assignee: u("4"),
    priority: p("low"),
    labels: l("documentation"),
    createdAt: "2026-02-04T14:00:00Z",
    cycleId: "cycle-4",
    project: pr("10"),
    rank: "0|hzzzzoms:",
  },
  {
    id: "21",
    identifier: "LNUI-301",
    title: "Responsive layout for tablet viewports",
    description: "Ensure the dashboard layout adapts gracefully to tablet-sized screens.",
    status: s("completed"),
    assignee: u("1"),
    priority: p("medium"),
    labels: l("ui", "design"),
    createdAt: "2026-02-05T08:00:00Z",
    cycleId: "cycle-4",
    project: pr("4"),
    rank: "0|hzzzzols:",
  },
  {
    id: "22",
    identifier: "LNUI-302",
    title: "Accessibility audit and WCAG 2.1 fixes",
    description: "Conduct full WCAG 2.1 AA audit and fix all identified accessibility issues.",
    status: s("technical-review"),
    assignee: u("2"),
    priority: p("high"),
    labels: l("accessibility"),
    createdAt: "2026-02-06T09:00:00Z",
    cycleId: "cycle-4",
    project: pr("2"),
    rank: "0|hzzzzok:",
    dueDate: "2026-03-30T00:00:00Z",
  },
  {
    id: "23",
    identifier: "LNUI-303",
    title: "Implement OAuth2 social login providers",
    description: "Add Google, GitHub, and Microsoft OAuth2 login flows to the auth service.",
    status: s("in-progress"),
    assignee: u("3"),
    priority: p("high"),
    labels: l("feature", "security"),
    createdAt: "2026-02-07T10:00:00Z",
    cycleId: "cycle-4",
    project: pr("3"),
    rank: "0|hzzzzoj:",
    dueDate: "2026-03-22T00:00:00Z",
  },
  {
    id: "24",
    identifier: "LNUI-304",
    title: "Dashboard widget customization",
    description: "Allow users to add, remove, and rearrange dashboard widgets.",
    status: s("backlog"),
    assignee: u("4"),
    priority: p("medium"),
    labels: l("feature", "ui"),
    createdAt: "2026-02-08T09:00:00Z",
    cycleId: "cycle-5",
    project: pr("4"),
    rank: "0|hzzzzoi:",
  },
  {
    id: "25",
    identifier: "LNUI-305",
    title: "Fix CORS policy for cross-origin requests",
    description: "API gateway is rejecting valid cross-origin requests from the web client.",
    status: s("to-do"),
    assignee: u("1"),
    priority: p("urgent"),
    labels: l("bug", "security"),
    createdAt: "2026-02-09T11:00:00Z",
    cycleId: "cycle-5",
    project: pr("5"),
    rank: "0|hzzzzoh:",
    dueDate: "2026-03-13T00:00:00Z",
  },
  {
    id: "26",
    identifier: "LNUI-406",
    title: "Elasticsearch index optimization",
    description:
      "Optimize search index mappings and analyzer configurations for better performance.",
    status: s("paused"),
    assignee: u("2"),
    priority: p("medium"),
    labels: l("performance"),
    createdAt: "2026-02-10T14:00:00Z",
    cycleId: "cycle-5",
    project: pr("6"),
    rank: "0|hzzzzog:",
  },
  {
    id: "27",
    identifier: "LNUI-427",
    title: "ETL pipeline error recovery mechanism",
    description: "Add checkpoint-based error recovery to prevent full pipeline reruns on failure.",
    status: s("completed"),
    assignee: u("3"),
    priority: p("high"),
    labels: l("feature", "refactor"),
    createdAt: "2026-02-11T08:00:00Z",
    cycleId: "cycle-5",
    project: pr("7"),
    rank: "0|hzzzzof:",
  },
  {
    id: "28",
    identifier: "LNUI-488",
    title: "Dark mode support for mobile app",
    description: "Implement system-aware dark mode with manual override in mobile settings.",
    status: s("technical-review"),
    assignee: u("4"),
    priority: p("medium"),
    labels: l("ui", "design"),
    createdAt: "2026-02-12T09:00:00Z",
    cycleId: "cycle-5",
    project: pr("8"),
    rank: "0|hzzzoe:",
    dueDate: "2026-04-05T00:00:00Z",
  },
  {
    id: "29",
    identifier: "LNUI-509",
    title: "Refactor CLI command parsing with cobra",
    description: "Migrate CLI command parsing from custom implementation to cobra library.",
    status: s("in-progress"),
    assignee: u("1"),
    priority: p("medium"),
    labels: l("refactor"),
    createdAt: "2026-02-13T10:00:00Z",
    cycleId: "cycle-5",
    project: pr("9"),
    rank: "0|hzzzod:",
  },
  {
    id: "30",
    identifier: "LNUI-526",
    title: "Add versioning to API documentation",
    description: "Support multiple API versions in documentation with version switcher UI.",
    status: s("backlog"),
    assignee: null,
    priority: p("no-priority"),
    labels: l("documentation"),
    createdAt: "2026-02-14T14:00:00Z",
    cycleId: "cycle-5",
    project: pr("10"),
    rank: "0|hzzzoc:",
  },
];

export function groupIssuesByStatus(issueList: Issue[]): Record<string, Issue[]> {
  return issueList.reduce(
    (acc, issue) => {
      const statusId = issue.status.id;
      if (!acc[statusId]) {
        acc[statusId] = [];
      }
      acc[statusId].push(issue);
      return acc;
    },
    {} as Record<string, Issue[]>,
  );
}

const priorityOrder: Record<string, number> = {
  urgent: 0,
  high: 1,
  medium: 2,
  low: 3,
  "no-priority": 4,
};

export function sortIssuesByPriority(issueList: Issue[]): Issue[] {
  return [...issueList].sort((a, b) => {
    const aOrder = priorityOrder[a.priority.id] ?? 99;
    const bOrder = priorityOrder[b.priority.id] ?? 99;
    return aOrder - bOrder;
  });
}
