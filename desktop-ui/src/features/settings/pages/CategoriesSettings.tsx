import { lazy, Suspense } from "react";

const CategoriesTab = lazy(() =>
  import("@features/system/components/tabs/CategoriesTab").then((m) => ({
    default: m.CategoriesTab,
  })),
);

export function CategoriesSettings() {
  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-foreground">Categories</h2>
        <p className="text-[13px] text-muted-foreground mt-1">
          Productivity categories and tracked applications
        </p>
      </div>
      <Suspense
        fallback={
          <div className="flex items-center justify-center h-32 text-muted-foreground text-sm">
            Loading...
          </div>
        }
      >
        <CategoriesTab />
      </Suspense>
    </div>
  );
}
