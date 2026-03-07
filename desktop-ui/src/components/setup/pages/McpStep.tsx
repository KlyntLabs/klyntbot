import { useOutletContext } from "react-router";
import type { SetupContext } from "../steps";

export function McpStep() {
  const { next } = useOutletContext<SetupContext>();

  return (
    <div>
      <h2 className="text-lg font-medium text-primary mb-2">MCP Servers</h2>
      <p className="text-[13px] text-muted mb-6">
        Connect external tools via the Model Context Protocol. You can add servers later in
        Settings.
      </p>
      <button
        type="button"
        onClick={next}
        className="px-5 py-2 text-[13px] font-medium text-white bg-brand hover:bg-brand-hover rounded-xl transition-colors"
      >
        Continue
      </button>
    </div>
  );
}
