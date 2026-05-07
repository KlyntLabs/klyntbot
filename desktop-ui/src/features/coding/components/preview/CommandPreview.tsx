import type { ApprovalPreview } from "./types";

type CommandProps = Extract<ApprovalPreview, { kind: "command" }>;

export function CommandPreview({ command, cwd, is_dangerous, risk_hits }: CommandProps) {
  return (
    <div className="approval-preview approval-preview--command">
      <header className="approval-preview__head">
        <span className="approval-preview__cwd">{cwd}</span>
        {is_dangerous && (
          <span className="approval-preview__badge approval-preview__badge--danger">
            dangerous
          </span>
        )}
      </header>
      <pre className="approval-preview__command">{command}</pre>
      {risk_hits.length > 0 && (
        <ul className="approval-preview__risks">
          {risk_hits.map((hit, idx) => (
            <li key={idx}>{hit}</li>
          ))}
        </ul>
      )}
    </div>
  );
}
