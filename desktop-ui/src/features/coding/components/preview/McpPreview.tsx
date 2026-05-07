import type { ApprovalPreview } from "./types";

type McpProps = Extract<ApprovalPreview, { kind: "mcp" }>;

export function McpPreview({ server, tool, args, schema }: McpProps) {
  return (
    <div className="approval-preview approval-preview--mcp">
      <header className="approval-preview__head">
        <span className="approval-preview__path">
          {server} / {tool}
        </span>
      </header>
      <pre className="approval-preview__command">{JSON.stringify(args, null, 2)}</pre>
      {schema && (
        <details>
          <summary>Schema</summary>
          <pre className="approval-preview__command">{JSON.stringify(schema, null, 2)}</pre>
        </details>
      )}
    </div>
  );
}
