import { useLocation, useNavigate } from "react-router";

interface CoachingLayoutProps {
  children: React.ReactNode;
}

const subNav = [
  { label: "Overview", path: "/coaching" },
  { label: "Patterns", path: "/coaching/patterns" },
  { label: "History", path: "/coaching/history" },
];

export function CoachingLayout({ children }: CoachingLayoutProps) {
  const navigate = useNavigate();
  const location = useLocation();
  const currentPath = location.pathname;

  return (
    <div className="flex-1 flex flex-col gap-2 overflow-hidden">
      <div className="h-12 flex items-center px-2 shrink-0">
        <div className="flex-1 flex items-center gap-1.5" role="tablist">
          {subNav.map((item) => {
            const isActive = currentPath === item.path;
            return (
              <button
                type="button"
                key={item.path}
                role="tab"
                aria-selected={isActive}
                onClick={() => navigate(item.path)}
                className={`flex-1 py-2 rounded-xl text-[13px] font-light transition-all duration-200 ${
                  isActive
                    ? "glass-button-active text-foreground"
                    : "text-muted-foreground hover:text-foreground hover:bg-accent"
                }`}
              >
                {item.label}
              </button>
            );
          })}
        </div>
      </div>
      <div className="flex-1 overflow-y-auto p-4">{children}</div>
    </div>
  );
}
