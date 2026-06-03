import Activity from "lucide-react/dist/esm/icons/activity";
import Folder from "lucide-react/dist/esm/icons/folder";
import GitBranch from "lucide-react/dist/esm/icons/git-branch";
import ScrollText from "lucide-react/dist/esm/icons/scroll-text";
import { type KeyboardEvent, type ReactNode, useRef } from "react";
import { cn } from "@/utils/cn";

export type PanelTabId = "activity" | "git" | "files" | "prompts";

type PanelTab = {
  id: PanelTabId;
  label: string;
  icon: ReactNode;
};

type PanelTabsProps = {
  active: PanelTabId;
  onSelect: (id: PanelTabId) => void;
  tabs?: PanelTab[];
};

const defaultTabs: PanelTab[] = [
  { id: "activity", label: "Activity", icon: <Activity aria-hidden /> },
  { id: "git", label: "Git", icon: <GitBranch aria-hidden /> },
  { id: "files", label: "Files", icon: <Folder aria-hidden /> },
  { id: "prompts", label: "Prompts", icon: <ScrollText aria-hidden /> },
];

export function PanelTabs({ active, onSelect, tabs = defaultTabs }: PanelTabsProps) {
  const tabRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const activeIndex = tabs.findIndex((tab) => tab.id === active);
  const focusableIndex = activeIndex >= 0 ? activeIndex : 0;

  const selectByIndex = (index: number, options?: { focus?: boolean }) => {
    if (tabs.length === 0) {
      return;
    }
    const normalized = (index + tabs.length) % tabs.length;
    onSelect(tabs[normalized].id);
    if (options?.focus) {
      tabRefs.current[normalized]?.focus();
    }
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    if (tabs.length <= 1) {
      return;
    }
    const currentIndex = activeIndex >= 0 ? activeIndex : index;
    if (event.key === "ArrowRight" || event.key === "ArrowDown") {
      event.preventDefault();
      selectByIndex(currentIndex + 1, { focus: true });
      return;
    }
    if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
      event.preventDefault();
      selectByIndex(currentIndex - 1, { focus: true });
      return;
    }
    if (event.key === "Home") {
      event.preventDefault();
      selectByIndex(0, { focus: true });
      return;
    }
    if (event.key === "End") {
      event.preventDefault();
      selectByIndex(tabs.length - 1, { focus: true });
    }
  };

  return (
    <div
      className="panel-tabs inline-flex items-center gap-[6px] rounded-full border border-border-subtle bg-surface-control p-[4px]"
      role="tablist"
      aria-label="Panel"
      aria-orientation="horizontal"
    >
      {tabs.map((tab, index) => {
        const isActive = active === tab.id;
        return (
          <button
            key={tab.id}
            type="button"
            className={cn(
              "panel-tab inline-flex items-center gap-0 rounded-full bg-transparent text-text-subtle shadow-none transition-[background,color] duration-ui-fast",
              isActive && "is-active",
            )}
            onClick={() => onSelect(tab.id)}
            onKeyDown={(event) => handleKeyDown(event, index)}
            ref={(element) => {
              tabRefs.current[index] = element;
            }}
            role="tab"
            aria-selected={isActive}
            tabIndex={index === focusableIndex ? 0 : -1}
            aria-label={tab.label}
            title={tab.label}
          >
            <span className="panel-tab-icon inline-flex items-center" aria-hidden>
              {tab.icon}
            </span>
          </button>
        );
      })}
    </div>
  );
}
