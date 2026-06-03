type WorkspaceGroupProps = {
  toggleId: string | null;
  name: string;
  showHeader: boolean;
  isCollapsed: boolean;
  onToggleCollapse: (groupId: string) => void;
  children: React.ReactNode;
};

export function WorkspaceGroup({
  toggleId,
  name,
  showHeader,
  isCollapsed,
  onToggleCollapse,
  children,
}: WorkspaceGroupProps) {
  const isToggleable = Boolean(toggleId);
  const headerContent = (
    <>
      <div className="text-ui-sm font-semibold tracking-wide text-text-strong">{name}</div>
      {isToggleable && (
        <span className={`group-toggle ${isCollapsed ? "" : "expanded"}`} aria-hidden>
          <span className="inline-block transition-transform duration-150">›</span>
        </span>
      )}
    </>
  );

  return (
    <div className="flex flex-col">
      {showHeader &&
        (isToggleable ? (
          <button
            type="button"
            className="flex items-center justify-between gap-2 px-1 pb-0.5 cursor-pointer focus-visible:outline focus-visible:outline-1 focus-visible:outline-border-subtle focus-visible:outline-offset-1 focus-visible:rounded-md"
            onClick={() => {
              if (toggleId) {
                onToggleCollapse(toggleId);
              }
            }}
            aria-label={isCollapsed ? "Expand group" : "Collapse group"}
            aria-expanded={!isCollapsed}
          >
            {headerContent}
          </button>
        ) : (
          <div className="flex items-center justify-between gap-2 px-1 pb-0.5">{headerContent}</div>
        ))}
      <div className={`workspace-group-list ${isCollapsed ? "collapsed" : ""}`}>
        <div className="workspace-group-content">{children}</div>
      </div>
    </div>
  );
}
