import { createHashRouter, RouterProvider } from "react-router";
import { MainApp } from "./components/views/MainApp";

function Placeholder({ name }: { name: string }) {
  return (
    <div className="h-screen w-screen bg-background text-foreground flex items-center justify-center">
      <p className="text-muted-foreground text-sm font-light">{name} — coming soon</p>
    </div>
  );
}

const router = createHashRouter([
  { path: "/", element: <MainApp /> },
  { path: "/chat", element: <Placeholder name="Chat" /> },
  { path: "/project/:id", element: <Placeholder name="Project Detail" /> },
  { path: "/launcher", element: <Placeholder name="Launcher" /> },
  { path: "/tray", element: <Placeholder name="System Tray" /> },
]);

export default function App() {
  return <RouterProvider router={router} />;
}
