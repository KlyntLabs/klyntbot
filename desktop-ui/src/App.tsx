import { lazy, Suspense } from "react";
import "./styles/index.css";

import MainApp from "@app/components/MainApp";
import { useWindowLabel } from "@/features/layout/hooks/useWindowLabel";
import { QueryProvider } from "@/lib/query";
import { currentWindowLabel } from "@/utils/tauri-bridge";

const AboutView = lazy(() =>
  import("@/features/about/components/AboutView").then((module) => ({
    default: module.AboutView,
  })),
);

const Launcher = lazy(() =>
  import("@/features/launcher/components/Launcher").then((module) => ({
    default: module.Launcher,
  })),
);

const Tray = lazy(() =>
  import("@/features/tray/components/Tray").then((module) => ({
    default: module.Tray,
  })),
);

const DistractionOverlay = lazy(() =>
  import("@/features/distraction/components/DistractionOverlay").then((module) => ({
    default: module.DistractionOverlay,
  })),
);

// Tauri 2 sets the label synchronously before any React render, so we read it
// once at module init from the real internals (the project-wide
// useWindowLabel hook is intercepted by a mock that always returns "main").
const realLabel = currentWindowLabel();

export default function App() {
  const windowLabel = useWindowLabel();

  if (realLabel === "launcher" || windowLabel === "launcher") {
    return (
      <QueryProvider>
        <Suspense fallback={null}>
          <Launcher />
        </Suspense>
      </QueryProvider>
    );
  }

  if (realLabel === "tray" || windowLabel === "tray") {
    return (
      <QueryProvider>
        <Suspense fallback={null}>
          <Tray />
        </Suspense>
      </QueryProvider>
    );
  }

  if (realLabel === "distraction-overlay" || windowLabel === "distraction-overlay") {
    return (
      <QueryProvider>
        <Suspense fallback={null}>
          <DistractionOverlay />
        </Suspense>
      </QueryProvider>
    );
  }

  if (windowLabel === "about") {
    return (
      <QueryProvider>
        <Suspense fallback={null}>
          <AboutView />
        </Suspense>
      </QueryProvider>
    );
  }

  return (
    <QueryProvider>
      <MainApp />
    </QueryProvider>
  );
}
