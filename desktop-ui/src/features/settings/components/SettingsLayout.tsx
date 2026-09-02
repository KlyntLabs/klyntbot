import { ToastContainer } from "@shared/components/ToastContainer";
import { ToastContextProvider, useToast } from "@shared/hooks/useToast";
import {
  Archive,
  ArrowLeft,
  BrainCircuit,
  Cable,
  Container,
  GitBranch,
  ListChecks,
  Mic,
  Plug,
  Rocket,
  SlidersHorizontal,
  User,
  Wrench,
} from "lucide-react";
import { useLocation, useNavigate } from "react-router";

interface SettingsLayoutProps {
  children: React.ReactNode;
}

const sections = [
  { label: "General", path: "/settings/general", icon: SlidersHorizontal },
  { label: "Configuration", path: "/settings/configuration", icon: Wrench },
  { label: "Personalization", path: "/settings/personalization", icon: User },
  { label: "Voice", path: "/settings/voice", icon: Mic },
  { label: "MCP servers", path: "/settings/mcp", icon: Plug },
  { label: "Git", path: "/settings/git", icon: GitBranch },
  { label: "Environments", path: "/settings/environments", icon: Container },
  { label: "Tasks & Notifications", path: "/settings/tasks", icon: ListChecks },
  { label: "Work Contexts", path: "/settings/work-contexts", icon: BrainCircuit },
  { label: "Launcher", path: "/settings/launcher", icon: Rocket },
  { label: "Integrations", path: "/settings/integrations", icon: Cable },
  { label: "Archived threads", path: "/settings/archived", icon: Archive },
];

export function SettingsLayout({ children }: SettingsLayoutProps) {
  const navigate = useNavigate();
  const location = useLocation();
  const currentPath = location.pathname;
  const toast = useToast();

  return (
    <ToastContextProvider value={{ show: toast.show }}>
      <ToastContainer toasts={toast.toasts} onDismiss={toast.dismiss} />

      {/* Settings sidebar — island pane */}
      <div className="w-56 island flex flex-col py-3">
        <button
          type="button"
          onClick={() => navigate("/")}
          className="flex items-center gap-2 px-4 py-1.5 text-ui text-fg-secondary hover:text-fg transition-colors mb-3"
        >
          <ArrowLeft className="size-3.5" />
          Back to app
        </button>

        <div className="px-3 mb-2">
          <span className="text-ui-xs font-medium text-fg-dim uppercase tracking-wider px-1">
            Settings
          </span>
        </div>

        <nav className="flex flex-col gap-0.5 px-2">
          {sections.map((section) => {
            const Icon = section.icon;
            const isActive = currentPath === section.path;
            return (
              <button
                type="button"
                key={section.path}
                onClick={() => navigate(section.path)}
                className={`flex items-center gap-2.5 px-2.5 py-1.5 rounded-row text-ui font-light transition-all duration-200 text-left ${
                  isActive
                    ? "bg-control-active text-fg"
                    : "text-fg-secondary hover:bg-control-hover hover:text-fg"
                }`}
              >
                <Icon className="size-4 flex-shrink-0" strokeWidth={1.5} />
                {section.label}
              </button>
            );
          })}
        </nav>
      </div>

      {/* Content area */}
      <div className="flex-1 overflow-y-auto island">
        <div className="max-w-3xl mx-auto p-8">{children}</div>
      </div>
    </ToastContextProvider>
  );
}
