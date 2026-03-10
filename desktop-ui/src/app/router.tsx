import { lazy, useEffect, useState } from "react";
import { createHashRouter, Navigate } from "react-router";
import { ipc } from "@shared/hooks/useIpc";
import { todayISO } from "@shared/lib/dates";
import type { AppInfoResponse } from "@shared/types";

// ── App Shell & Layouts ───────────────────────────────────────────
const AppShell = lazy(() =>
  import("./layouts/AppShell").then((m) => ({ default: m.AppShell })),
);

// ── Tasks Feature ─────────────────────────────────────────────────
const TasksPage = lazy(() =>
  import("../features/tasks").then((m) => ({ default: m.TasksPage })),
);
const ProjectDetailPage = lazy(() =>
  import("../features/tasks").then((m) => ({ default: m.ProjectDetailPage })),
);
const TaskDetailPage = lazy(() =>
  import("../features/tasks").then((m) => ({ default: m.TaskDetailPage })),
);
const ObjectiveDetailPage = lazy(() =>
  import("../features/tasks").then((m) => ({ default: m.ObjectiveDetailPage })),
);

// ── Chat Feature ──────────────────────────────────────────────────
const ChatPage = lazy(() =>
  import("../features/chat").then((m) => ({ default: m.ChatPage })),
);

// ── Notes Feature ────────────────────────────────────────────────
const NotesPage = lazy(() =>
  import("../features/notes").then((m) => ({ default: m.NotesPage })),
);

// ── Finance Feature ──────────────────────────────────────────────
const FinanceOverviewPage = lazy(() =>
  import("../features/finance").then((m) => ({ default: m.FinanceOverviewPage })),
);
const AccountsPage = lazy(() =>
  import("../features/finance").then((m) => ({ default: m.AccountsPage })),
);
const TransactionsPage = lazy(() =>
  import("../features/finance").then((m) => ({ default: m.TransactionsPage })),
);
const BudgetsPage = lazy(() =>
  import("../features/finance").then((m) => ({ default: m.BudgetsPage })),
);
const InvestmentsPage = lazy(() =>
  import("../features/finance").then((m) => ({ default: m.InvestmentsPage })),
);
const GoalsPage = lazy(() =>
  import("../features/finance").then((m) => ({ default: m.GoalsPage })),
);
const LiabilitiesPage = lazy(() =>
  import("../features/finance").then((m) => ({ default: m.LiabilitiesPage })),
);

// ── Productivity Feature ──────────────────────────────────────────
const CategoriesPage = lazy(() =>
  import("../features/productivity").then((m) => ({ default: m.CategoriesPage })),
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

// ── Debug Feature ────────────────────────────────────────────────
const DebugDashboardPage = lazy(() =>
  import("../features/debug").then((m) => ({ default: m.DebugDashboardPage })),
);

// ── Tray Feature ─────────────────────────────────────────────────
const LauncherPage = lazy(() =>
  import("../features/tray").then((m) => ({ default: m.LauncherPage })),
);
const SystemTrayPage = lazy(() =>
  import("../features/tray").then((m) => ({ default: m.SystemTrayPage })),
);

// ── Distraction Feature ──────────────────────────────────────────
const DistractionOverlay = lazy(() =>
  import("../features/distraction").then((m) => ({ default: m.DistractionOverlay })),
);

// ── Setup Wizard ─────────────────────────────────────────────────
const SetupLayout = lazy(() =>
  import("../features/setup").then((m) => ({ default: m.SetupLayout })),
);
const WelcomeStep = lazy(() =>
  import("../features/setup").then((m) => ({ default: m.WelcomeStep })),
);
const ProviderStep = lazy(() =>
  import("../features/setup").then((m) => ({ default: m.ProviderStep })),
);
const ChannelsStep = lazy(() =>
  import("../features/setup").then((m) => ({ default: m.ChannelsStep })),
);
const AreasStep = lazy(() =>
  import("../features/setup").then((m) => ({ default: m.AreasStep })),
);
const ProductivityStep = lazy(() =>
  import("../features/setup").then((m) => ({ default: m.ProductivityStep })),
);
const FinanceStep = lazy(() =>
  import("../features/setup").then((m) => ({ default: m.FinanceStep })),
);
const McpStep = lazy(() =>
  import("../features/setup").then((m) => ({ default: m.McpStep })),
);
const CompleteStep = lazy(() =>
  import("../features/setup").then((m) => ({ default: m.CompleteStep })),
);

// ── Redirect Components ──────────────────────────────────────────
function DashboardRedirect() {
  return <Navigate to={`/day/${todayISO()}`} replace />;
}

function SetupRedirect() {
  const [target, setTarget] = useState<string | null>(null);

  useEffect(() => {
    ipc<AppInfoResponse>("app_info")
      .then((info) => setTarget(info.setupCompleted ? "/" : "/setup/welcome"))
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
      {
        path: "/categories",
        element: (
          <DashboardLayout>
            <CategoriesPage />
          </DashboardLayout>
        ),
      },
      { path: "/tasks", element: <TasksPage /> },
      { path: "/chat", element: <ChatPage /> },
      { path: "/notes", element: <NotesPage /> },
      { path: "/project/:id", element: <ProjectDetailPage /> },
      { path: "/task/:id", element: <TaskDetailPage /> },
      { path: "/objective/:id", element: <ObjectiveDetailPage /> },
      // Redirect old productivity routes to dashboard
      { path: "/productivity", element: <Navigate to="/" replace /> },
      { path: "/productivity/day/:date", element: <Navigate to="/" replace /> },
      { path: "/productivity/week/:weekStart", element: <Navigate to="/" replace /> },
      { path: "/productivity/month/:yearMonth", element: <Navigate to="/" replace /> },
      { path: "/productivity/categories", element: <Navigate to="/categories" replace /> },
      { path: "/finance", element: <FinanceOverviewPage /> },
      { path: "/finance/accounts", element: <AccountsPage /> },
      { path: "/finance/transactions", element: <TransactionsPage /> },
      { path: "/finance/budgets", element: <BudgetsPage /> },
      { path: "/finance/investments", element: <InvestmentsPage /> },
      { path: "/finance/goals", element: <GoalsPage /> },
      { path: "/finance/liabilities", element: <LiabilitiesPage /> },
      { path: "/debug", element: <DebugDashboardPage /> },
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
        path: "/settings/archived",
        element: (
          <SettingsLayout>
            <ArchivedSettings />
          </SettingsLayout>
        ),
      },
    ],
  },
  {
    path: "/setup",
    element: <SetupLayout />,
    children: [
      { index: true, element: <Navigate to="/setup/welcome" replace /> },
      { path: "welcome", element: <WelcomeStep /> },
      { path: "provider", element: <ProviderStep /> },
      { path: "channels", element: <ChannelsStep /> },
      { path: "areas", element: <AreasStep /> },
      { path: "productivity", element: <ProductivityStep /> },
      { path: "finance", element: <FinanceStep /> },
      { path: "mcp", element: <McpStep /> },
      { path: "complete", element: <CompleteStep /> },
    ],
  },
  { path: "/launcher", element: <LauncherPage /> },
  { path: "/tray", element: <SystemTrayPage /> },
  { path: "/distraction-overlay", element: <DistractionOverlay /> },
  { path: "*", element: <SetupRedirect /> },
]);
