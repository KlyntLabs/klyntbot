import { lazy, Suspense } from "react";

const ContextsTab = lazy(() =>
  import("@features/system/components/tabs/ContextsTab").then((m) => ({
    default: m.ContextsTab,
  })),
);

export function ContextsDetail() {
  return (
    <Suspense
      fallback={
        <div className="flex items-center justify-center h-32 text-muted-foreground text-sm">
          Loading...
        </div>
      }
    >
      <ContextsTab />
    </Suspense>
  );
}
