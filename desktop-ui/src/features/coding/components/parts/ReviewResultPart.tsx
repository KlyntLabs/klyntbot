import type { ReviewIssue } from "@/bindings";
import { openWorkspaceIn } from "@/api/endpoints/workspace";

type Props = {
  reviewId: string;
  summary: string;
  issues: ReviewIssue[];
};

const SEVERITY_ORDER: Array<ReviewIssue["severity"]> = ["error", "warning", "info"];

export function ReviewResultPart({ reviewId, summary, issues }: Props) {
  const grouped = groupBySeverity(issues);

  const openFile = async (file: string, line: number | null) => {
    await openWorkspaceIn(file, { line: line ?? null });
  };

  return (
    <article className="review-result-part" data-review-id={reviewId}>
      <header className="review-result-part__summary">{summary}</header>
      {issues.length === 0 && <p className="review-result-part__empty">No issues found.</p>}
      {SEVERITY_ORDER.map((sev) => {
        const items = grouped[sev] ?? [];
        if (items.length === 0) return null;
        return (
          <section key={sev} className={`review-result-part__group review-result-part__group--${sev}`}>
            <h4>{labelFor(sev)} <span className="count">{items.length}</span></h4>
            <ol>
              {items.map((issue, idx) => (
                <li key={`${sev}-${idx}`}>
                  {issue.file && (
                    <button type="button" className="review-issue__location"
                      onClick={() => openFile(issue.file!, issue.line)}>
                      {issue.file}{issue.line != null ? `:${issue.line}` : ""}
                    </button>
                  )}
                  <p className="review-issue__description">{issue.description}</p>
                  {issue.suggestion && (
                    <p className="review-issue__suggestion">→ {issue.suggestion}</p>
                  )}
                </li>
              ))}
            </ol>
          </section>
        );
      })}
    </article>
  );
}

function groupBySeverity(issues: ReviewIssue[]): Record<string, ReviewIssue[]> {
  const out: Record<string, ReviewIssue[]> = {};
  for (const i of issues) {
    out[i.severity] = out[i.severity] ?? [];
    out[i.severity].push(i);
  }
  return out;
}

function labelFor(sev: string): string {
  return { error: "Errors", warning: "Warnings", info: "Info" }[sev] ?? sev;
}
