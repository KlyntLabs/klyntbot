import { useShallow } from "zustand/react/shallow";
import { useTabStore } from "../../store/tab-store";

export function IssueDetailBreadcrumb() {
  const navStack = useTabStore(
    useShallow((s) => {
      const active = s.tabs.find((t) => t.id === s.activeTabId);
      return active?.navStack ?? [];
    }),
  );
  const navigateToStackIndex = useTabStore((s) => s.navigateToStackIndex);

  return (
    <div className="flex items-center gap-1 mb-3 min-w-0">
      {navStack.map((entry, index) => {
        const isLast = index === navStack.length - 1;
        return (
          <div key={`${entry.type}-${entry.targetId}`} className="flex items-center gap-1 min-w-0">
            {index > 0 && <span className="text-xs text-muted shrink-0">›</span>}
            {isLast ? (
              <span className="text-xs font-medium text-primary truncate">{entry.label}</span>
            ) : (
              <button
                type="button"
                onClick={() => navigateToStackIndex(index)}
                className="text-xs text-muted hover:text-primary transition-colors truncate"
              >
                {entry.label}
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
}
