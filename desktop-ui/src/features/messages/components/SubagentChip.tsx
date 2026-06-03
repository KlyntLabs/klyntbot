type Props = {
  agentId: string;
  sessionId: string;
  description?: string;
};

export function SubagentChip({ agentId, sessionId, description }: Props) {
  if (!sessionId) return null;
  return (
    <button
      type="button"
      className="subagent-chip"
      onClick={() => {
        // Emit a custom event that the app shell can listen for to switch threads.
        window.dispatchEvent(
          new CustomEvent("klynt:navigate-to-thread", {
            detail: { sessionId },
          }),
        );
      }}
      title={`Open subagent ${agentId}`}
    >
      <span className="opacity-60">↳</span>
      <span className="font-mono">{agentId}</span>
      {description && <span className="opacity-50">—</span>}
      {description && <span className="italic">{description}</span>}
    </button>
  );
}
