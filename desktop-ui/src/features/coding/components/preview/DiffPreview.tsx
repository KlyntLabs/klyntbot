import { useMemo } from "react";
import type { ApprovalPreview } from "./types";

type DiffProps = Extract<ApprovalPreview, { kind: "diff" }>;

export function DiffPreview({
  path,
  unified_diff,
  lines_added,
  lines_removed,
  is_new_file,
  is_truncated,
}: DiffProps) {
  const lines = useMemo(() => unified_diff.split("\n"), [unified_diff]);
  return (
    <div className="approval-preview approval-preview--diff">
      <header className="approval-preview__head">
        <span className="approval-preview__path">{path}</span>
        {is_new_file && (
          <span className="approval-preview__badge approval-preview__badge--new">new file</span>
        )}
        <span className="approval-preview__lines-added">+{lines_added}</span>
        <span className="approval-preview__lines-removed">-{lines_removed}</span>
      </header>
      <pre className="approval-preview__diff">
        {lines.map((line, idx) => (
          <DiffLine key={`${idx}-${line}`} text={line} />
        ))}
      </pre>
      {is_truncated && (
        <footer className="approval-preview__truncated">
          Truncated - approve to see full diff in the editor.
        </footer>
      )}
    </div>
  );
}

function DiffLine({ text }: { text: string }) {
  let className = "approval-preview__diff-line";
  if (text.startsWith("+++") || text.startsWith("---")) {
    className += " approval-preview__diff-line--filehead";
  } else if (text.startsWith("@@")) {
    className += " approval-preview__diff-line--hunk";
  } else if (text.startsWith("+")) {
    className += " approval-preview__diff-line--added";
  } else if (text.startsWith("-")) {
    className += " approval-preview__diff-line--removed";
  }
  return <span className={className}>{text || " "}</span>;
}
