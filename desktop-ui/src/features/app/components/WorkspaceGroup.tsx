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
      <div className="workspace-group-label">{name}</div>
      {isToggleable && (
        <span className={`group-toggle ${isCollapsed ? "" : "expanded"}`} aria-hidden>
          <span className="group-toggle-icon">›</span>
        </span>
      )}
    </>
  );

  return (
    <div className="workspace-group">
      {showHeader &&
        (isToggleable ? (
          <button
            type="button"
            className="workspace-group-header is-toggleable"
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
          <div className="workspace-group-header">{headerContent}</div>
        ))}
      <div className={`workspace-group-list ${isCollapsed ? "collapsed" : ""}`}>
        <div className="workspace-group-content">{children}</div>
      </div>
    </div>
  );
}
