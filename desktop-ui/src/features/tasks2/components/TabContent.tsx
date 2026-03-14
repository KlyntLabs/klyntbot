import { useShallow } from "zustand/react/shallow";
import { useTabStore } from "../store/tab-store";
import AllIssues from "./AllIssues";
import { AreaView } from "./AreaView";
import { IssueDetailErrorBoundary } from "./detail/IssueDetailError";
import { IssueDetailView } from "./detail/IssueDetailView";
import HeaderNav from "./HeaderNav";
import HeaderOptions from "./HeaderOptions";
import { ProjectView } from "./ProjectView";

export function TabContent() {
  const activeTab = useTabStore(
    useShallow((s) => s.tabs.find((t) => t.id === s.activeTabId) ?? null),
  );

  if (!activeTab) {
    return (
      <div className="flex-1 flex items-center justify-center text-sm text-[hsl(var(--muted-foreground))]">
        Click + to open a tab
      </div>
    );
  }

  const currentView =
    activeTab.navStack.length > 0 ? activeTab.navStack[activeTab.navStack.length - 1] : null;

  if (!currentView) {
    return (
      <div className="flex-1 flex items-center justify-center text-sm text-[hsl(var(--muted-foreground))]">
        Click + to open a tab
      </div>
    );
  }

  const renderContent = () => {
    switch (currentView.type) {
      case "my-issues":
        return <AllIssues />;
      case "all-issues":
        return <AllIssues />;
      case "area":
        return <AreaView areaId={currentView.targetId} />;
      case "project":
        return <ProjectView projectId={currentView.targetId} />;
      case "issue":
        return (
          <IssueDetailErrorBoundary>
            <IssueDetailView issueId={currentView.targetId} />
          </IssueDetailErrorBoundary>
        );
    }
  };

  return (
    <>
      <HeaderNav />
      <HeaderOptions />
      <div className="overflow-auto w-full flex-1 min-w-0">{renderContent()}</div>
    </>
  );
}
