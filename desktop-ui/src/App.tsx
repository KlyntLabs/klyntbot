import { createHashRouter, RouterProvider } from "react-router";
import { MainApp } from "./components/views/MainApp";
import { Chat } from "./components/views/Chat";
import { ProjectDetail } from "./components/views/ProjectDetail";
import { TaskDetail } from "./components/views/TaskDetail";
import { ObjectiveDetail } from "./components/views/ObjectiveDetail";
import { Finance } from "./components/views/Finance";
import { FinanceAccounts } from "./components/views/FinanceAccounts";
import { FinanceTransactions } from "./components/views/FinanceTransactions";
import { FinanceBudgets } from "./components/views/FinanceBudgets";
import { FinanceInvestments } from "./components/views/FinanceInvestments";
import { FinanceGoals } from "./components/views/FinanceGoals";
import { FinanceLiabilities } from "./components/views/FinanceLiabilities";
import { Launcher } from "./components/views/Launcher";
import { SystemTray } from "./components/views/SystemTray";

const router = createHashRouter([
  { path: "/", element: <MainApp /> },
  { path: "/chat", element: <Chat /> },
  { path: "/project/:id", element: <ProjectDetail /> },
  { path: "/task/:id", element: <TaskDetail /> },
  { path: "/objective/:id", element: <ObjectiveDetail /> },
  { path: "/finance", element: <Finance /> },
  { path: "/finance/accounts", element: <FinanceAccounts /> },
  { path: "/finance/transactions", element: <FinanceTransactions /> },
  { path: "/finance/budgets", element: <FinanceBudgets /> },
  { path: "/finance/investments", element: <FinanceInvestments /> },
  { path: "/finance/goals", element: <FinanceGoals /> },
  { path: "/finance/liabilities", element: <FinanceLiabilities /> },
  { path: "/launcher", element: <Launcher /> },
  { path: "/tray", element: <SystemTray /> },
]);

export default function App() {
  return <RouterProvider router={router} />;
}
