import { createHashRouter, Navigate, RouterProvider } from "react-router";
import { ProductivityDayPage } from "./components/productivity/pages/DayPage";
import { ProductivityMonthPage } from "./components/productivity/pages/MonthPage";
import { ProductivityWeekPage } from "./components/productivity/pages/WeekPage";
import { ArchivedSettings } from "./components/settings/pages/ArchivedSettings";
import { ConfigurationSettings } from "./components/settings/pages/ConfigurationSettings";
import { EnvironmentsSettings } from "./components/settings/pages/EnvironmentsSettings";
import { GeneralSettings } from "./components/settings/pages/GeneralSettings";
import { GitSettings } from "./components/settings/pages/GitSettings";
import { McpServersSettings } from "./components/settings/pages/McpServersSettings";
import { PersonalizationSettings } from "./components/settings/pages/PersonalizationSettings";
import { SettingsLayout } from "./components/settings/SettingsLayout";
import { Chat } from "./components/views/Chat";
import { Finance } from "./components/views/Finance";
import { FinanceAccounts } from "./components/views/FinanceAccounts";
import { FinanceBudgets } from "./components/views/FinanceBudgets";
import { FinanceGoals } from "./components/views/FinanceGoals";
import { FinanceInvestments } from "./components/views/FinanceInvestments";
import { FinanceLiabilities } from "./components/views/FinanceLiabilities";
import { FinanceTransactions } from "./components/views/FinanceTransactions";
import { Launcher } from "./components/views/Launcher";
import { MainApp } from "./components/views/MainApp";
import { ObjectiveDetail } from "./components/views/ObjectiveDetail";
import { ProjectDetail } from "./components/views/ProjectDetail";
import { SystemTray } from "./components/views/SystemTray";
import { TaskDetail } from "./components/views/TaskDetail";
import { DistractionOverlay } from "./components/distraction/DistractionOverlay";

const router = createHashRouter([
  { path: "/", element: <MainApp /> },
  { path: "/chat", element: <Chat /> },
  { path: "/project/:id", element: <ProjectDetail /> },
  { path: "/task/:id", element: <TaskDetail /> },
  { path: "/objective/:id", element: <ObjectiveDetail /> },
  {
    path: "/productivity",
    element: <Navigate to={`/productivity/day/${new Date().toISOString().slice(0, 10)}`} replace />,
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
  return <RouterProvider router={router} />;
}
