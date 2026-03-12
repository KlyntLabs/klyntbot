import "../tasks2.css";
import AllIssues from "../components/AllIssues";
import { CreateIssueModal } from "../components/CreateIssueModal";
import HeaderNav from "../components/HeaderNav";
import HeaderOptions from "../components/HeaderOptions";
import { PortalContainerProvider } from "../components/portal-context";
import { Tasks2Layout } from "../components/Tasks2Layout";

export function Tasks2Page() {
  return (
    <PortalContainerProvider>
      <div className="tasks2-scope flex-1 h-full min-w-0">
        <Tasks2Layout>
          <HeaderNav />
          <HeaderOptions />
          <div className="overflow-auto w-full flex-1 min-w-0">
            <AllIssues />
          </div>
        </Tasks2Layout>
        <CreateIssueModal />
      </div>
    </PortalContainerProvider>
  );
}
