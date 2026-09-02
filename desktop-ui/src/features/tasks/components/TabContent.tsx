import { useShallow } from "zustand/react/shallow";
import type { UseTasksResult } from "../hooks/useTasks";
import { useTabStore } from "../store/tab-store";
import AllIssues from "./AllIssues";
import { AreaView } from "./AreaView";
import { IssueDetailErrorBoundary } from "./detail/IssueDetailError";
import { IssueDetailView } from "./detail/IssueDetailView";
import HeaderNav from "./HeaderNav";
import HeaderOptions from "./HeaderOptions";
import { ProjectView } from "./ProjectView";

interface TabContentProps {
  tasksData: UseTasksResult;
}

export function TabContent({ tasksData }: TabContentProps) {
  const activeTab = useTabStore(
    useShallow((s) => s.tabs.find((t) => t.id === s.activeTabId) ?? null),
  );

  if (!activeTab) {
    return (
      <div className="flex-1 flex items-center justify-center text-sm text-fg-secondary">
        Click + to open a tab
      </div>
    );
  }

  const currentView =
    activeTab.navStack.length > 0 ? activeTab.navStack[activeTab.navStack.length - 1] : null;

  if (!currentView) {
    return (
      <div className="flex-1 flex items-center justify-center text-sm text-fg-secondary">
        Click + to open a tab
      </div>
    );
  }

  const renderContent = () => {
    switch (currentView.type) {
      case "my-issues":
        return <AllIssues tasksData={tasksData} />;
      case "all-issues":
        return <AllIssues tasksData={tasksData} />;
      case "area":
        return <AreaView areaId={currentView.targetId} tasksData={tasksData} />;
      case "project":
        return <ProjectView projectId={currentView.targetId} tasksData={tasksData} />;
      case "issue":
        return (
          <IssueDetailErrorBoundary>
            <IssueDetailView
              issueId={currentView.targetId}
              projectMap={tasksData.projectMap}
              areaMap={tasksData.areaMap}
            />
          </IssueDetailErrorBoundary>
        );
    }
  };

  return (
    <>
      <HeaderNav />
      <HeaderOptions issues={tasksData.issues} projects={tasksData.projects} />
      <div className="overflow-auto w-full flex-1 min-w-0">{renderContent()}</div>
    </>
  );
}
