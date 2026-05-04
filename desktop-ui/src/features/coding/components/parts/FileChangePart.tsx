export function FileChangePart({
  path,
  diffUnified,
  applied,
}: {
  path: string;
  diffUnified: string;
  applied: boolean;
}) {
  return (
    <div
      className={`part-file-change ${applied ? "part-file-change--applied" : "part-file-change--pending"}`}
    >
      <div className="part-file-change__header">
        <span className="part-file-change__path">{path}</span>
        <span className="part-file-change__status">{applied ? "Applied" : "Pending"}</span>
      </div>
      {diffUnified && <pre className="part-file-change__diff">{diffUnified}</pre>}
    </div>
  );
}
