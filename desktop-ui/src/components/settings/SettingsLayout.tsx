import {
  Archive,
  ArrowLeft,
  Container,
  GitBranch,
  Plug,
  SlidersHorizontal,
  User,
  Wrench,
} from "lucide-react";
import { useLocation, useNavigate } from "react-router";
import { Sidebar } from "../layout/Sidebar";

interface SettingsLayoutProps {
  children: React.ReactNode;
}

const sections = [
  { label: "General", path: "/settings/general", icon: SlidersHorizontal },
  { label: "Configuration", path: "/settings/configuration", icon: Wrench },
  { label: "Personalization", path: "/settings/personalization", icon: User },
  { label: "MCP servers", path: "/settings/mcp", icon: Plug },
  { label: "Git", path: "/settings/git", icon: GitBranch },
  { label: "Environments", path: "/settings/environments", icon: Container },
  { label: "Archived threads", path: "/settings/archived", icon: Archive },
];

export function SettingsLayout({ children }: SettingsLayoutProps) {
  const navigate = useNavigate();
  const location = useLocation();
  const currentPath = location.pathname;

  return (
    <div className="h-screen w-screen bg-background text-primary flex overflow-hidden">
      <Sidebar
        active="Settings"
        onNavigate={(item) => {
          if (item === "Tasks") navigate("/");
          if (item === "Chat") navigate("/chat");
          if (item === "Finance") navigate("/finance");
          if (item === "Settings") navigate("/settings");
        }}
      />

      {/* Settings sidebar */}
      <div className="w-56 bg-background border-r border-border flex flex-col py-3">
        <button
          onClick={() => navigate("/")}
          className="flex items-center gap-2 px-4 py-1.5 text-[13px] text-muted hover:text-secondary transition-colors mb-3"
        >
          <ArrowLeft className="w-3.5 h-3.5" />
          Back to app
        </button>

        <div className="px-3 mb-2">
          <span className="text-[11px] font-medium text-dim uppercase tracking-wider px-1">
            Settings
          </span>
        </div>

        <nav className="flex flex-col gap-0.5 px-2">
          {sections.map((section) => {
            const Icon = section.icon;
            const isActive = currentPath === section.path;
            return (
              <button
                key={section.path}
                onClick={() => navigate(section.path)}
                className={`flex items-center gap-2.5 px-2.5 py-1.5 rounded-md text-[13px] font-light transition-colors text-left ${
                  isActive
                    ? "bg-surface-highest text-primary"
                    : "text-muted hover:bg-surface-base hover:text-secondary"
                }`}
              >
                <Icon className="w-4 h-4 flex-shrink-0" strokeWidth={1.5} />
                {section.label}
              </button>
            );
          })}
        </nav>
      </div>

      {/* Content area */}
      <div className="flex-1 overflow-y-auto">
        <div className="max-w-3xl mx-auto p-8">{children}</div>
      </div>
    </div>
  );
}
