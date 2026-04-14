import { ipc } from "@shared/hooks/useIpc";
import { todayISO } from "@shared/lib/dates";
import type { AppInfoResponse } from "@shared/types";
import { lazy, useEffect, useState } from "react";
import { createHashRouter, Navigate } from "react-router";

// ── App Shell & Layouts ───────────────────────────────────────────
const AppShell = lazy(() => import("./layouts/AppShell").then((m) => ({ default: m.AppShell })));

// ── Automations Feature ──────────────────────────────────────────
const AutomationsPage = lazy(() =>
  import("../features/automations").then((m) => ({ default: m.AutomationsPage })),
);

// ── Chat Feature ──────────────────────────────────────────────────
const ChatPage = lazy(() => import("../features/chat").then((m) => ({ default: m.ChatPage })));

// ── Notes Feature ────────────────────────────────────────────────
const KnowledgeBasePage = lazy(() =>
  import("../features/notes").then((m) => ({ default: m.KnowledgeBasePage })),
);

// ── Learn Feature ────────────────────────────────────────────────
const LearnPage = lazy(() => import("../features/learn").then((m) => ({ default: m.LearnPage })));
const KnowledgeHealthPage = lazy(() =>
  import("../features/learn").then((m) => ({ default: m.KnowledgeHealth })),
);
const FocusedReviewPage = lazy(() =>
  import("../features/learn").then((m) => ({ default: m.FocusedReview })),
);

// ── Brain Feature (consolidated) ─────────────────────────────
const BrainPage = lazy(() => import("../features/brain").then((m) => ({ default: m.BrainPage })));

// ── Projects Feature ─────────────────────────────────────────────
const ProjectDetailPage = lazy(() =>
  import("../features/projects").then((m) => ({ default: m.ProjectDetailPage })),
);
const ProjectsListPage = lazy(() =>
  import("../features/projects").then((m) => ({ default: m.ProjectsListPage })),
);

// ── Dashboard Feature ────────────────────────────────────────────
const DashboardLayout = lazy(() =>
  import("../features/dashboard").then((m) => ({ default: m.DashboardLayout })),
);
const DayCalendarView = lazy(() =>
  import("../features/dashboard").then((m) => ({ default: m.DayCalendarView })),
);
const WeekCalendarView = lazy(() =>
  import("../features/dashboard").then((m) => ({ default: m.WeekCalendarView })),
);
const MonthCalendarView = lazy(() =>
  import("../features/dashboard").then((m) => ({ default: m.MonthCalendarView })),
);
const YearHeatmapView = lazy(() =>
  import("../features/dashboard").then((m) => ({ default: m.YearHeatmapView })),
);

// ── Settings Feature ─────────────────────────────────────────────
const SettingsLayout = lazy(() =>
  import("../features/settings").then((m) => ({ default: m.SettingsLayout })),
);
const GeneralSettings = lazy(() =>
  import("../features/settings").then((m) => ({ default: m.GeneralSettings })),
);
const AiSettings = lazy(() =>
  import("../features/settings").then((m) => ({ default: m.AiSettings })),
);
const CategoriesSettings = lazy(() =>
  import("../features/settings").then((m) => ({ default: m.CategoriesSettings })),
);
const ConfigurationSettings = lazy(() =>
  import("../features/settings").then((m) => ({ default: m.ConfigurationSettings })),
);
const PersonalizationSettings = lazy(() =>
  import("../features/settings").then((m) => ({ default: m.PersonalizationSettings })),
);
const McpServersSettings = lazy(() =>
  import("../features/settings").then((m) => ({ default: m.McpServersSettings })),
);
const GitSettings = lazy(() =>
  import("../features/settings").then((m) => ({ default: m.GitSettings })),
);
const EnvironmentsSettings = lazy(() =>
  import("../features/settings").then((m) => ({ default: m.EnvironmentsSettings })),
);
const ArchivedSettings = lazy(() =>
  import("../features/settings").then((m) => ({ default: m.ArchivedSettings })),
);
const IntegrationsSettings = lazy(() =>
  import("../features/settings").then((m) => ({ default: m.IntegrationsSettings })),
);
const TasksSettings = lazy(() =>
  import("../features/settings").then((m) => ({ default: m.TasksSettings })),
);
const LauncherSettings = lazy(() =>
  import("../features/settings").then((m) => ({ default: m.LauncherSettings })),
);
const WorkContextSettings = lazy(() =>
  import("../features/settings").then((m) => ({ default: m.WorkContextSettings })),
);
const VoiceSettings = lazy(() =>
  import("../features/settings").then((m) => ({ default: m.VoiceSettings })),
);

// (Debug feature — now integrated into System page)

// ── Tray Feature ─────────────────────────────────────────────────
const LauncherPage = lazy(() =>
  import("../features/tray").then((m) => ({ default: m.LauncherPage })),
);
const SystemTrayPage = lazy(() =>
  import("../features/tray").then((m) => ({ default: m.SystemTrayPage })),
);

// ── Voice Feature ───────────────────────────────────────────────
const VoiceOrbPage = lazy(() => import("@features/voice/pages/VoiceOrbPage"));

// ── Distraction Feature ──────────────────────────────────────────
const DistractionOverlay = lazy(() =>
  import("../features/distraction").then((m) => ({ default: m.DistractionOverlay })),
);

// ── Setup Wizard ─────────────────────────────────────────────────
const ConversationRunner = lazy(() =>
  import("../features/setup").then((m) => ({ default: m.ConversationRunner })),
);

// ── Redirect Components ──────────────────────────────────────────
function DashboardRedirect() {
  return <Navigate to={`/day/${todayISO()}`} replace />;
}

function SetupRedirect() {
  const [target, setTarget] = useState<string | null>(null);

  useEffect(() => {
    ipc<AppInfoResponse>("app_info")
      .then((info) => setTarget(info.setupCompleted ? "/" : "/setup"))
      .catch(() => setTarget("/"));
  }, []);

  if (!target) return null;
  return <Navigate to={target} replace />;
}

// ── Router Definition ────────────────────────────────────────────
export const router = createHashRouter([
  {
    element: <AppShell />,
    children: [
      { path: "/", element: <DashboardRedirect /> },
      {
        path: "/day/:date",
        element: (
          <DashboardLayout>
            <DayCalendarView />
          </DashboardLayout>
        ),
      },
      {
        path: "/week/:date",
        element: (
          <DashboardLayout>
            <WeekCalendarView />
          </DashboardLayout>
        ),
      },
      {
        path: "/month/:date",
        element: (
          <DashboardLayout>
            <MonthCalendarView />
          </DashboardLayout>
        ),
      },
      {
        path: "/year/:year",
        element: (
          <DashboardLayout>
            <YearHeatmapView />
          </DashboardLayout>
        ),
      },
      { path: "/chat", element: <ChatPage /> },
      { path: "/notes", element: <KnowledgeBasePage /> },
      { path: "/learn", element: <LearnPage /> },
      { path: "/learn/knowledge", element: <KnowledgeHealthPage /> },
      { path: "/learn/review/:topicId?", element: <FocusedReviewPage /> },
      { path: "/brain", element: <BrainPage /> },
      { path: "/brain/:section", element: <BrainPage /> },
      { path: "/automations", element: <AutomationsPage /> },
      { path: "/categories", element: <Navigate to="/settings/categories" replace /> },
      { path: "/coaching", element: <Navigate to="/brain/coaching" replace /> },
      { path: "/coaching/*", element: <Navigate to="/brain/coaching" replace /> },
      { path: "/system", element: <Navigate to="/brain/contexts" replace /> },
      { path: "/system/memory", element: <Navigate to="/brain/memory" replace /> },
      { path: "/system/contexts", element: <Navigate to="/brain/contexts" replace /> },
      { path: "/system/events", element: <Navigate to="/brain" replace /> },
      { path: "/system/pipeline", element: <Navigate to="/brain" replace /> },
      { path: "/system/inference", element: <Navigate to="/brain/contexts" replace /> },
      { path: "/system/categories", element: <Navigate to="/settings/categories" replace /> },
      { path: "/debug", element: <Navigate to="/brain" replace /> },
      { path: "/settings", element: <Navigate to="/settings/general" replace /> },
      {
        path: "/settings/general",
        element: (
          <SettingsLayout>
            <GeneralSettings />
          </SettingsLayout>
        ),
      },
      {
        path: "/settings/ai",
        element: (
          <SettingsLayout>
            <AiSettings />
          </SettingsLayout>
        ),
      },
      {
        path: "/settings/categories",
        element: (
          <SettingsLayout>
            <CategoriesSettings />
          </SettingsLayout>
        ),
      },
      {
        path: "/settings/configuration",
        element: (
          <SettingsLayout>
            <ConfigurationSettings />
          </SettingsLayout>
        ),
      },
      {
        path: "/settings/personalization",
        element: (
          <SettingsLayout>
            <PersonalizationSettings />
          </SettingsLayout>
        ),
      },
      {
        path: "/settings/voice",
        element: (
          <SettingsLayout>
            <VoiceSettings />
          </SettingsLayout>
        ),
      },
      {
        path: "/settings/mcp",
        element: (
          <SettingsLayout>
            <McpServersSettings />
          </SettingsLayout>
        ),
      },
      {
        path: "/settings/git",
        element: (
          <SettingsLayout>
            <GitSettings />
          </SettingsLayout>
        ),
      },
      {
        path: "/settings/environments",
        element: (
          <SettingsLayout>
            <EnvironmentsSettings />
          </SettingsLayout>
        ),
      },
      {
        path: "/settings/integrations",
        element: (
          <SettingsLayout>
            <IntegrationsSettings />
          </SettingsLayout>
        ),
      },
      {
        path: "/settings/tasks",
        element: (
          <SettingsLayout>
            <TasksSettings />
          </SettingsLayout>
        ),
      },
      {
        path: "/settings/work-contexts",
        element: (
          <SettingsLayout>
            <WorkContextSettings />
          </SettingsLayout>
        ),
      },
      {
        path: "/settings/launcher",
        element: (
          <SettingsLayout>
            <LauncherSettings />
          </SettingsLayout>
        ),
      },
      {
        path: "/settings/archived",
        element: (
          <SettingsLayout>
            <ArchivedSettings />
          </SettingsLayout>
        ),
      },
      {
        path: "db/:databaseId",
        lazy: () =>
          import("@features/database/pages/DatabasePage").then((m) => ({ Component: m.default })),
      },
      {
        path: "skills",
        lazy: async () => ({
          Component: (await import("@features/skills/pages/SkillsListPage")).SkillsListPage,
        }),
      },
      {
        path: "skills/:source",
        lazy: async () => ({
          Component: (await import("@features/skills/pages/SkillDetailPage")).SkillDetailPage,
        }),
      },
      { path: "/projects", element: <ProjectsListPage /> },
      { path: "/project/:id", element: <ProjectDetailPage /> },
      { path: "/project/:id/:tab", element: <ProjectDetailPage /> },
    ],
  },
  { path: "/setup", element: <ConversationRunner /> },
  { path: "/setup/*", element: <ConversationRunner /> },
  { path: "/launcher", element: <LauncherPage /> },
  { path: "/tray", element: <SystemTrayPage /> },
  { path: "/voice-orb", element: <VoiceOrbPage /> },
  { path: "/distraction-overlay", element: <DistractionOverlay /> },
  { path: "*", element: <SetupRedirect /> },
]);
