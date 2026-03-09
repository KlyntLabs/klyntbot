import { lazy, Suspense, useEffect, useState } from "react";
import { createHashRouter, Navigate, RouterProvider } from "react-router";
import { ipc } from "./hooks/useIpc";
import { todayISO } from "./lib/dates";
import type { AppInfoResponse } from "./lib/types";

const AppShell = lazy(() =>
  import("./components/layout/AppShell").then((m) => ({ default: m.AppShell })),
);
const MainApp = lazy(() =>
  import("./components/views/MainApp").then((m) => ({ default: m.MainApp })),
);
const Chat = lazy(() => import("./components/views/Chat").then((m) => ({ default: m.Chat })));
const ProjectDetail = lazy(() =>
  import("./components/views/ProjectDetail").then((m) => ({ default: m.ProjectDetail })),
);
const TaskDetail = lazy(() =>
  import("./components/views/TaskDetail").then((m) => ({ default: m.TaskDetail })),
);
const ObjectiveDetail = lazy(() =>
  import("./components/views/ObjectiveDetail").then((m) => ({ default: m.ObjectiveDetail })),
);
const ProductivityDayPage = lazy(() =>
  import("./components/productivity/pages/DayPage").then((m) => ({
    default: m.ProductivityDayPage,
  })),
);
const ProductivityWeekPage = lazy(() =>
  import("./components/productivity/pages/WeekPage").then((m) => ({
    default: m.ProductivityWeekPage,
  })),
);
const ProductivityMonthPage = lazy(() =>
  import("./components/productivity/pages/MonthPage").then((m) => ({
    default: m.ProductivityMonthPage,
  })),
);
const CategoriesPage = lazy(() =>
  import("./components/productivity/pages/CategoriesPage").then((m) => ({
    default: m.CategoriesPage,
  })),
);
const NotesView = lazy(() => import("./components/notes/NotesView"));
const Finance = lazy(() =>
  import("./components/views/Finance").then((m) => ({ default: m.Finance })),
);
const FinanceAccounts = lazy(() =>
  import("./components/views/FinanceAccounts").then((m) => ({ default: m.FinanceAccounts })),
);
const FinanceTransactions = lazy(() =>
  import("./components/views/FinanceTransactions").then((m) => ({
    default: m.FinanceTransactions,
  })),
);
const FinanceBudgets = lazy(() =>
  import("./components/views/FinanceBudgets").then((m) => ({ default: m.FinanceBudgets })),
);
const FinanceInvestments = lazy(() =>
  import("./components/views/FinanceInvestments").then((m) => ({ default: m.FinanceInvestments })),
);
const FinanceGoals = lazy(() =>
  import("./components/views/FinanceGoals").then((m) => ({ default: m.FinanceGoals })),
);
const FinanceLiabilities = lazy(() =>
  import("./components/views/FinanceLiabilities").then((m) => ({ default: m.FinanceLiabilities })),
);
const DebugDashboard = lazy(() =>
  import("./components/debug/DebugDashboard").then((m) => ({ default: m.DebugDashboard })),
);
const SettingsLayout = lazy(() =>
  import("./components/settings/SettingsLayout").then((m) => ({ default: m.SettingsLayout })),
);
const GeneralSettings = lazy(() =>
  import("./components/settings/pages/GeneralSettings").then((m) => ({
    default: m.GeneralSettings,
  })),
);
const ConfigurationSettings = lazy(() =>
  import("./components/settings/pages/ConfigurationSettings").then((m) => ({
    default: m.ConfigurationSettings,
  })),
);
const PersonalizationSettings = lazy(() =>
  import("./components/settings/pages/PersonalizationSettings").then((m) => ({
    default: m.PersonalizationSettings,
  })),
);
const McpServersSettings = lazy(() =>
  import("./components/settings/pages/McpServersSettings").then((m) => ({
    default: m.McpServersSettings,
  })),
);
const GitSettings = lazy(() =>
  import("./components/settings/pages/GitSettings").then((m) => ({ default: m.GitSettings })),
);
const EnvironmentsSettings = lazy(() =>
  import("./components/settings/pages/EnvironmentsSettings").then((m) => ({
    default: m.EnvironmentsSettings,
  })),
);
const ArchivedSettings = lazy(() =>
  import("./components/settings/pages/ArchivedSettings").then((m) => ({
    default: m.ArchivedSettings,
  })),
);
const DashboardLayout = lazy(() =>
  import("./components/dashboard/DashboardLayout").then((m) => ({
    default: m.DashboardLayout,
  })),
);
const DashboardDayPage = lazy(() =>
  import("./components/dashboard/DayCalendarView").then((m) => ({
    default: m.DayCalendarView,
  })),
);
const DashboardWeekPage = lazy(() =>
  import("./components/dashboard/WeekCalendarView").then((m) => ({
    default: m.WeekCalendarView,
  })),
);
const DashboardMonthPage = lazy(() =>
  import("./components/dashboard/MonthCalendarView").then((m) => ({
    default: m.MonthCalendarView,
  })),
);
const DashboardYearPage = lazy(() =>
  import("./components/dashboard/YearHeatmapView").then((m) => ({
    default: m.YearHeatmapView,
  })),
);
const Launcher = lazy(() =>
  import("./components/views/Launcher").then((m) => ({ default: m.Launcher })),
);
const SystemTray = lazy(() =>
  import("./components/views/SystemTray").then((m) => ({ default: m.SystemTray })),
);
const DistractionOverlay = lazy(() =>
  import("./components/distraction/DistractionOverlay").then((m) => ({
    default: m.DistractionOverlay,
  })),
);
// ── Setup Wizard ──────────────────────────────────────────────────────
const SetupLayout = lazy(() =>
  import("./components/setup/SetupLayout").then((m) => ({ default: m.SetupLayout })),
);
const WelcomeStep = lazy(() =>
  import("./components/setup/pages/WelcomeStep").then((m) => ({ default: m.WelcomeStep })),
);
const ProviderStep = lazy(() =>
  import("./components/setup/pages/ProviderStep").then((m) => ({ default: m.ProviderStep })),
);
const ChannelsStep = lazy(() =>
  import("./components/setup/pages/ChannelsStep").then((m) => ({ default: m.ChannelsStep })),
);
const AreasStep = lazy(() =>
  import("./components/setup/pages/AreasStep").then((m) => ({ default: m.AreasStep })),
);
const ProductivityStep = lazy(() =>
  import("./components/setup/pages/ProductivityStep").then((m) => ({
    default: m.ProductivityStep,
  })),
);
const FinanceStep = lazy(() =>
  import("./components/setup/pages/FinanceStep").then((m) => ({ default: m.FinanceStep })),
);
const McpStep = lazy(() =>
  import("./components/setup/pages/McpStep").then((m) => ({ default: m.McpStep })),
);
const CompleteStep = lazy(() =>
  import("./components/setup/pages/CompleteStep").then((m) => ({ default: m.CompleteStep })),
);

function DashboardRedirect() {
  return <Navigate to={`/day/${todayISO()}`} replace />;
}

function ProductivityRedirect() {
  return <Navigate to={`/productivity/day/${todayISO()}`} replace />;
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

const router = createHashRouter([
  {
    element: <AppShell />,
    children: [
      { path: "/", element: <DashboardRedirect /> },
      {
        path: "/day/:date",
        element: (
          <DashboardLayout>
            <DashboardDayPage />
          </DashboardLayout>
        ),
      },
      {
        path: "/week/:date",
        element: (
          <DashboardLayout>
            <DashboardWeekPage />
          </DashboardLayout>
        ),
      },
      {
        path: "/month/:date",
        element: (
          <DashboardLayout>
            <DashboardMonthPage />
          </DashboardLayout>
        ),
      },
      {
        path: "/year/:year",
        element: (
          <DashboardLayout>
            <DashboardYearPage />
          </DashboardLayout>
        ),
      },
      { path: "/tasks", element: <MainApp /> },
      { path: "/chat", element: <Chat /> },
      { path: "/notes", element: <NotesView /> },
      { path: "/project/:id", element: <ProjectDetail /> },
      { path: "/task/:id", element: <TaskDetail /> },
      { path: "/objective/:id", element: <ObjectiveDetail /> },
      { path: "/productivity", element: <ProductivityRedirect /> },
      { path: "/productivity/day/:date", element: <ProductivityDayPage /> },
      { path: "/productivity/week/:weekStart", element: <ProductivityWeekPage /> },
      { path: "/productivity/month/:yearMonth", element: <ProductivityMonthPage /> },
      { path: "/productivity/categories", element: <CategoriesPage /> },
      { path: "/finance", element: <Finance /> },
      { path: "/finance/accounts", element: <FinanceAccounts /> },
      { path: "/finance/transactions", element: <FinanceTransactions /> },
      { path: "/finance/budgets", element: <FinanceBudgets /> },
      { path: "/finance/investments", element: <FinanceInvestments /> },
      { path: "/finance/goals", element: <FinanceGoals /> },
      { path: "/finance/liabilities", element: <FinanceLiabilities /> },
      { path: "/debug", element: <DebugDashboard /> },
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
  { path: "/launcher", element: <Launcher /> },
  { path: "/tray", element: <SystemTray /> },
  { path: "/distraction-overlay", element: <DistractionOverlay /> },
  { path: "*", element: <SetupRedirect /> },
]);

export default function App() {
  return (
    <Suspense fallback={null}>
      <RouterProvider router={router} />
    </Suspense>
  );
}
