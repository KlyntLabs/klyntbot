import { lazy, Suspense } from "react";
import { createHashRouter, Navigate, RouterProvider } from "react-router";
import { todayISO } from "./lib/dates";

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

function ProductivityRedirect() {
  return <Navigate to={`/productivity/day/${todayISO()}`} replace />;
}

const router = createHashRouter([
  { path: "/", element: <MainApp /> },
  { path: "/chat", element: <Chat /> },
  { path: "/project/:id", element: <ProjectDetail /> },
  { path: "/task/:id", element: <TaskDetail /> },
  { path: "/objective/:id", element: <ObjectiveDetail /> },
  {
    path: "/productivity",
    element: <ProductivityRedirect />,
  },
  { path: "/productivity/day/:date", element: <ProductivityDayPage /> },
  { path: "/productivity/week/:weekStart", element: <ProductivityWeekPage /> },
  { path: "/productivity/month/:yearMonth", element: <ProductivityMonthPage /> },
  { path: "/finance", element: <Finance /> },
  { path: "/finance/accounts", element: <FinanceAccounts /> },
  { path: "/finance/transactions", element: <FinanceTransactions /> },
  { path: "/finance/budgets", element: <FinanceBudgets /> },
  { path: "/finance/investments", element: <FinanceInvestments /> },
  { path: "/finance/goals", element: <FinanceGoals /> },
  { path: "/finance/liabilities", element: <FinanceLiabilities /> },
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
  { path: "/launcher", element: <Launcher /> },
  { path: "/tray", element: <SystemTray /> },
  { path: "/distraction-overlay", element: <DistractionOverlay /> },
  { path: "*", element: <Navigate to="/" replace /> },
]);

export default function App() {
  return (
    <Suspense fallback={null}>
      <RouterProvider router={router} />
    </Suspense>
  );
}
