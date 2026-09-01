import { Suspense } from "react";
import { RouterProvider } from "react-router";
import { ThemeProvider } from "./app/providers/ThemeProvider";
import { router } from "./app/router";
import { ErrorBoundary } from "./shared/components/ErrorBoundary";

export default function App() {
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
