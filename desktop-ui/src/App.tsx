import { Suspense, useEffect } from "react";
import { RouterProvider } from "react-router";
import { ThemeProvider } from "./app/providers/ThemeProvider";
import { router } from "./app/router";
import { ErrorBoundary } from "./shared/components/ErrorBoundary";
import { checkForUpdates } from "./shared/lib/updater";

export default function App() {
  useEffect(() => {
    const timer = setTimeout(() => {
      checkForUpdates();
    }, 3000);
    return () => clearTimeout(timer);
  }, []);

  return (
    <ErrorBoundary>
      <ThemeProvider>
        <Suspense fallback={null}>
          <RouterProvider router={router} />
        </Suspense>
      </ThemeProvider>
    </ErrorBoundary>
  );
}
