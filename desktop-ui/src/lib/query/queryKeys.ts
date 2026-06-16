// Single source of truth for query keys. Add new domains here, never inline
// raw arrays at callsites — see docs/superpowers/plans/2026-04-26-realtime-
// data-layer-phase-1.md "Type consistency" section for why.
export const qk = {
  tasks: {
    all: () => ["tasks"] as const,
    today: () => ["tasks", "today"] as const,
    byId: (id: string) => ["tasks", "byId", id] as const,
  },
  calendar: {
    all: () => ["calendar"] as const,
    eventsForDate: (date: string) => ["calendar", "events", date] as const,
  },
  focus: {
    all: () => ["focus"] as const,
    status: () => ["focus", "status"] as const,
    todaySessions: () => ["focus", "todaySessions"] as const,
    defaults: () => ["focus", "defaults"] as const,
  },
  flashcards: {
    all: () => ["flashcards"] as const,
    dueCount: () => ["flashcards", "dueCount"] as const,
  },
  launcher: {
    all: () => ["launcher"] as const,
    dashboard: () => ["launcher", "dashboard"] as const,
    search: (query: string) => ["launcher", "search", query] as const,
    dndActive: () => ["launcher", "dndActive"] as const,
    pinned: () => ["launcher", "pinned"] as const,
  },
  settings: {
    all: () => ["settings"] as const,
    app: () => ["settings", "app"] as const,
    codexConfigPath: () => ["settings", "codexConfigPath"] as const,
    features: (workspaceId: string | null) =>
      ["settings", "features", workspaceId ?? "global"] as const,
    tailscaleStatus: () => ["settings", "tailscaleStatus"] as const,
    tailscaleCommandPreview: () => ["settings", "tailscaleCommandPreview"] as const,
    tcpDaemonStatus: () => ["settings", "tcpDaemonStatus"] as const,
    workspaces: () => ["settings", "workspaces"] as const,
    defaultModels: (workspaceIds: string) => ["settings", "defaultModels", workspaceIds] as const,
  },
  agents: {
    all: () => ["agents"] as const,
    settings: () => ["agents", "settings"] as const,
    configToml: (agentName: string) => ["agents", "configToml", agentName] as const,
  },
  models: {
    all: () => ["models"] as const,
    list: (workspaceId: string) => ["models", "list", workspaceId] as const,
    configModel: (workspaceId: string) => ["models", "configModel", workspaceId] as const,
  },
  skills: {
    all: () => ["skills"] as const,
    list: (workspaceId: string) => ["skills", "list", workspaceId] as const,
  },
  apps: {
    all: () => ["apps"] as const,
    list: (workspaceId: string, threadId: string | null) =>
      ["apps", "list", workspaceId, threadId ?? "no-thread"] as const,
  },
  prompts: {
    all: () => ["prompts"] as const,
    list: (workspaceId: string) => ["prompts", "list", workspaceId] as const,
  },
  git: {
    all: () => ["git"] as const,
    status: (workspaceId: string) => ["git", "status", workspaceId] as const,
    branches: (workspaceId: string) => ["git", "branches", workspaceId] as const,
    diffs: (workspaceId: string) => ["git", "diffs", workspaceId] as const,
    log: (workspaceId: string) => ["git", "log", workspaceId] as const,
    remote: (workspaceId: string) => ["git", "remote", workspaceId] as const,
    commitDiffs: (workspaceId: string, sha: string) =>
      ["git", "commitDiffs", workspaceId, sha] as const,
    repoScan: (workspaceId: string, depth: number) =>
      ["git", "repoScan", workspaceId, depth] as const,
  },
  github: {
    all: () => ["github"] as const,
    issues: (workspaceId: string) => ["github", "issues", workspaceId] as const,
    pulls: (workspaceId: string) => ["github", "pulls", workspaceId] as const,
    diffsForPr: (workspaceId: string, n: number) =>
      ["github", "pulls", workspaceId, n, "diffs"] as const,
    commentsForPr: (workspaceId: string, n: number) =>
      ["github", "pulls", workspaceId, n, "comments"] as const,
  },
  threads: {
    all: () => ["threads"] as const,
    list: () => ["threads", "list"] as const,
    byId: (id: string) => ["threads", "byId", id] as const,
  },
  system: {
    all: () => ["system"] as const,
    mcpServers: () => ["system", "mcpServers"] as const,
  },
  dashboard: {
    all: () => ["dashboard"] as const,
    timeline: (startDate: string, endDate: string, sources: readonly string[]) =>
      ["dashboard", "timeline", startDate, endDate, [...sources].sort().join(",")] as const,
    productivityToday: (date: string) => ["dashboard", "productivityToday", date] as const,
    intelligence: (date: string) => ["dashboard", "intelligence", date] as const,
  },
  calendarSync: {
    all: () => ["calendarSync"] as const,
    status: () => ["calendarSync", "status"] as const,
  },
  productivity: {
    all: () => ["productivity"] as const,
    calendarEvents: (date: string) => ["productivity", "calendarEvents", date] as const,

    summaryRange: (startDate: string, endDate: string) =>
      ["productivity", "summaryRange", startDate, endDate] as const,
    weekly: () => ["productivity", "weekly"] as const,
    patterns: (days: number | null) => ["productivity", "patterns", days ?? "default"] as const,
    hourlyBreakdown: (startDate: string, endDate: string) =>
      ["productivity", "hourlyBreakdown", startDate, endDate] as const,
    timeline: (date: string) => ["productivity", "timeline", date] as const,
    categories: () => ["productivity", "categories"] as const,
    intelligenceSessions: (date: string) => ["productivity", "intelligenceSessions", date] as const,
    activityFeed: (limit: number) => ["productivity", "activityFeed", limit] as const,
    goals: () => ["productivity", "goals"] as const,
  },
} as const;

type QkType = typeof qk;
type Domain = QkType[keyof QkType];
type Factory = Domain[keyof Domain];
export type QueryKey = ReturnType<Factory>;
