import { createBrowserRouter, RouterProvider } from "react-router";

function Placeholder({ name }: { name: string }) {
  return (
    <div className="h-screen w-screen bg-[#0E0E0D] text-[#E6EDF3] flex items-center justify-center">
      <p className="text-[#8B949E] text-sm font-light">{name} — coming soon</p>
    </div>
  );
}

const router = createBrowserRouter([
  { path: "/", element: <Placeholder name="Tasks" /> },
  { path: "/chat", element: <Placeholder name="Chat" /> },
  { path: "/project/:id", element: <Placeholder name="Project Detail" /> },
  { path: "/launcher", element: <Placeholder name="Launcher" /> },
  { path: "/tray", element: <Placeholder name="System Tray" /> },
]);

export default function App() {
  return <RouterProvider router={router} />;
}
