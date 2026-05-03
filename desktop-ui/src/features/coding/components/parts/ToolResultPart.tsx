import type { ToolOutput } from "./types";

export function ToolResultPart({
  callId,
  output,
  isError,
}: {
  callId: string;
  output: ToolOutput;
  isError: boolean;
}) {
  return (
    <div className={`part-tool-result ${isError ? "part-tool-result--error" : ""}`}>
      <div className="part-tool-result__header">
        <span className="part-tool-result__label">
          {isError ? "Error" : "Result"} for {callId.slice(0, 8)}
        </span>
        {output.truncated && <span className="part-tool-result__truncated">(truncated)</span>}
      </div>
      <pre className="part-tool-result__content">{output.text}</pre>
    </div>
  );
}
