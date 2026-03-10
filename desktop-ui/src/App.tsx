import { Suspense } from "react";
import { RouterProvider } from "react-router";
import { ThemeProvider } from "./app/providers/ThemeProvider";
import { router } from "./app/router";

export default function App() {
  return (
    <ThemeProvider>
      <Suspense fallback={null}>
        <RouterProvider router={router} />
      </Suspense>
    </ThemeProvider>
  );
}
