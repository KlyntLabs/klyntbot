// Stub: AI chat is intentionally not wired yet. Once the new desktop-ui exposes
// a chat session API, replace this with a real implementation that mirrors the
// .bak's LauncherChat behaviour.

interface Props {
  initialQuery: string | null;
  onBack: () => void;
  onExpand: () => void;
}

export function LauncherChatStub({ initialQuery, onBack, onExpand }: Props) {
  return (
    <div className="lc-chat-stub">
      <div className="lc-chat-stub-header">
        <button type="button" onClick={onBack} className="lc-chat-stub-back">
          ← Back
        </button>
        <span className="lc-chat-stub-title">Klynt AI</span>
        <button type="button" onClick={onExpand} className="lc-chat-stub-expand">
          Expand ↗
        </button>
      </div>
      <div className="lc-chat-stub-body">
        <p className="lc-muted-sm">AI chat will hook into the new project's session API.</p>
        {initialQuery && (
          <p className="lc-chat-stub-query">
            Query: <em>{initialQuery}</em>
          </p>
        )}
        <p className="lc-dim-xs">Press Esc to go back.</p>
      </div>
    </div>
  );
}
