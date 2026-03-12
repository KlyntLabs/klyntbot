import "../tasks2.css";
import { CreateIssueModal } from "../components/CreateIssueModal";
import { PortalContainerProvider } from "../components/portal-context";
import { TabBar } from "../components/TabBar";
import { TabContent } from "../components/TabContent";
import { Tasks2Layout } from "../components/Tasks2Layout";

export function Tasks2Page() {
  return (
    <PortalContainerProvider>
      <div className="tasks2-scope flex-1 h-full min-w-0">
        <Tasks2Layout>
          <TabBar />
          <TabContent />
        </Tasks2Layout>
        <CreateIssueModal />
      </div>
    </PortalContainerProvider>
  );
}
