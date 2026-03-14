import { useState } from "react";
import type { useIssueDetail } from "../../hooks/useIssueDetail";
import { cn } from "../../lib/utils";
import { IssueActivityTab } from "./IssueActivityTab";
import { IssueContentTab } from "./IssueContentTab";

type TabId = "content" | "activity";

interface IssueDetailTabsProps {
  detail: ReturnType<typeof useIssueDetail>;
}

const tabs: { id: TabId; label: string }[] = [
  { id: "content", label: "Content" },
  { id: "activity", label: "Activity Log" },
];

export function IssueDetailTabs({ detail }: IssueDetailTabsProps) {
  const [activeTab, setActiveTab] = useState<TabId>("content");

  return (
    <div className="flex flex-col min-h-0">
      <div className="flex gap-4 border-b border-[hsl(var(--border))] mb-4">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            type="button"
            onClick={() => setActiveTab(tab.id)}
            className={cn(
              "pb-2 text-sm font-medium transition-colors border-b-2 -mb-px",
              activeTab === tab.id
                ? "border-[hsl(var(--foreground))] text-[hsl(var(--foreground))]"
                : "border-transparent text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))]",
            )}
          >
            {tab.label}
          </button>
        ))}
      </div>
      {activeTab === "content" ? (
        <IssueContentTab detail={detail} />
      ) : (
        <IssueActivityTab activity={detail.activity} />
      )}
    </div>
  );
}
