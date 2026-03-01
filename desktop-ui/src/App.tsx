import { createHashRouter, RouterProvider } from "react-router";
import { MainApp } from "./components/views/MainApp";
import { Chat } from "./components/views/Chat";
import { ProjectDetail } from "./components/views/ProjectDetail";
import { Launcher } from "./components/views/Launcher";
import { SystemTray } from "./components/views/SystemTray";

const router = createHashRouter([
  { path: "/", element: <MainApp /> },
  { path: "/chat", element: <Chat /> },
  { path: "/project/:id", element: <ProjectDetail /> },
  { path: "/launcher", element: <Launcher /> },
  { path: "/tray", element: <SystemTray /> },
]);

export default function App() {
  return <RouterProvider router={router} />;
}
