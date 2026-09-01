import type React from "react";

export function TasksLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="h-full overflow-hidden flex flex-col bg-background min-w-0">
      <div className="border border-border rounded-md overflow-hidden flex flex-col h-full min-w-0">
        {children}
      </div>
    </div>
  );
}
