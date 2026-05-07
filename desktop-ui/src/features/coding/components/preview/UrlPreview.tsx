import type { ApprovalPreview } from "./types";

type UrlProps = Extract<ApprovalPreview, { kind: "url" }>;

export function UrlPreview({ method, url, headers, body_preview }: UrlProps) {
  return (
    <div className="approval-preview approval-preview--url">
      <header className="approval-preview__head">
        <span className="approval-preview__badge">{method}</span>
        <span className="approval-preview__path">{url}</span>
      </header>
      {headers.length > 0 && (
        <dl className="approval-preview__headers">
          {headers.map(([k, v]) => (
            <div key={k}>
              <dt>{k}</dt>
              <dd>{v}</dd>
            </div>
          ))}
        </dl>
      )}
      {body_preview && <pre className="approval-preview__command">{body_preview}</pre>}
    </div>
  );
}
