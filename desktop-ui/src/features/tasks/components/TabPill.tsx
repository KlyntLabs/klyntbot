import { X } from "lucide-react";
import { memo } from "react";
import type { Tab } from "../store/tab-store";
import { useTabStore } from "../store/tab-store";

interface TabPillProps {
  tab: Tab;
  isActive: boolean;
}

export const TabPill = memo(function TabPill({ tab, isActive }: TabPillProps) {
  const setActiveTab = useTabStore((s) => s.setActiveTab);
  const closeTab = useTabStore((s) => s.closeTab);

  const label = tab.navStack.map((entry) => entry.label).join(" \u203A ");

  return (
    <div
      onClick={() => setActiveTab(tab.id)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") setActiveTab(tab.id);
      }}
      role="tab"
      tabIndex={0}
      aria-selected={isActive}
      className={`group flex items-center gap-1.5 whitespace-nowrap rounded-t-lg px-3.5 py-1.5 text-ui transition-colors cursor-pointer ${
        isActive ? "bg-control-hover text-fg" : "text-fg-secondary hover:text-fg"
      }`}
    >
      <span className="truncate max-w-[200px]">{label}</span>
      <button
        type="button"
        tabIndex={-1}
        onClick={(e) => {
          e.stopPropagation();
          closeTab(tab.id);
        }}
        className="text-fg-secondary hover:text-fg transition-colors"
      >
        <X className="h-3 w-3" />
      </button>
    </div>
  );
});
