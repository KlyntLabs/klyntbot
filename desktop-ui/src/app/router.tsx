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

// ── Tasks Feature ─────────────────────────────────────────────────
const TasksPage = lazy(() => import("../features/tasks").then((m) => ({ default: m.TasksPage })));

// ── Chat Feature ──────────────────────────────────────────────────
const ChatPage = lazy(() => import("../features/chat").then((m) => ({ default: m.ChatPage })));

// ── Notes Feature ────────────────────────────────────────────────
const KnowledgeBasePage = lazy(() =>
  import("../features/notes").then((m) => ({ default: m.KnowledgeBasePage })),
);

// ── Learn Feature ────────────────────────────────────────────────
const LearnPage = lazy(() => import("../features/learn").then((m) => ({ default: m.LearnPage })));

// ── Finance Feature ──────────────────────────────────────────────
const FinanceOverviewPage = lazy(() =>
  import("../features/finance").then((m) => ({ default: m.FinanceOverviewPage })),
);
const CashFlowPage = lazy(() =>
  import("../features/finance").then((m) => ({ default: m.CashFlowPage })),
);
const InvestmentsPage = lazy(() =>
  import("../features/finance").then((m) => ({ default: m.InvestmentsPage })),
);
const TargetsPage = lazy(() =>
  import("../features/finance").then((m) => ({ default: m.TargetsPage })),
);

// ── Projects Feature ─────────────────────────────────────────────
const ProjectDetailPage = lazy(() =>
  import("../features/projects").then((m) => ({ default: m.ProjectDetailPage })),
);
const ProjectsListPage = lazy(() =>
  import("../features/projects").then((m) => ({ default: m.ProjectsListPage })),
);

// ── System Feature ───────────────────────────────────────────────
const SystemPage = lazy(() =>
  import("../features/system").then((m) => ({ default: m.SystemPage })),
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

// (Debug feature — now integrated into System page)

// ── Tray Feature ─────────────────────────────────────────────────
const LauncherPage = lazy(() =>
  import("../features/tray").then((m) => ({ default: m.LauncherPage })),
);
const SystemTrayPage = lazy(() =>
  import("../features/tray").then((m) => ({ default: m.SystemTrayPage })),
);

// ── Quick Capture ────────────────────────────────────────────────
const QuickCapturePage = lazy(() =>
  import("../features/notes").then((m) => ({ default: m.QuickCapturePage })),
);

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
      // System page (contexts, categories, inference, debug tabs)
      { path: "/system", element: <SystemPage /> },
      { path: "/system/:tab", element: <SystemPage /> },
      { path: "/tasks", element: <TasksPage /> },
      { path: "/chat", element: <ChatPage /> },
      { path: "/notes", element: <KnowledgeBasePage /> },
      { path: "/learn", element: <LearnPage /> },
      { path: "/automations", element: <AutomationsPage /> },
      // Redirect old routes
      { path: "/productivity", element: <Navigate to="/" replace /> },
      { path: "/productivity/day/:date", element: <Navigate to="/" replace /> },
      { path: "/productivity/week/:weekStart", element: <Navigate to="/" replace /> },
      { path: "/productivity/month/:yearMonth", element: <Navigate to="/" replace /> },
      { path: "/productivity/categories", element: <Navigate to="/system/categories" replace /> },
      { path: "/categories", element: <Navigate to="/system/categories" replace /> },
      { path: "/finance", element: <FinanceOverviewPage /> },
      { path: "/finance/cashflow", element: <CashFlowPage /> },
      { path: "/finance/accounts", element: <Navigate to="/finance/cashflow" replace /> },
      { path: "/finance/transactions", element: <Navigate to="/finance/cashflow" replace /> },
      { path: "/finance/budgets", element: <Navigate to="/finance/cashflow" replace /> },
      { path: "/finance/investments", element: <InvestmentsPage /> },
      { path: "/finance/targets", element: <TargetsPage /> },
      { path: "/finance/goals", element: <Navigate to="/finance/targets" replace /> },
      { path: "/finance/liabilities", element: <Navigate to="/finance/targets" replace /> },
      { path: "/debug", element: <Navigate to="/system/events" replace /> },
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
        path: "/settings/archived",
        element: (
          <SettingsLayout>
            <ArchivedSettings />
          </SettingsLayout>
        ),
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
  { path: "/quick-capture", element: <QuickCapturePage /> },
  { path: "/distraction-overlay", element: <DistractionOverlay /> },
  { path: "*", element: <SetupRedirect /> },
]);
