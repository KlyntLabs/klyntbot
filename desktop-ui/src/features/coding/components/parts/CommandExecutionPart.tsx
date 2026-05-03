function statusClass(exitCode: number | null): "success" | "error" | "running" {
  if (exitCode === 0) return "success";
  if (exitCode !== null) return "error";
  return "running";
}

export function CommandExecutionPart({
  command,
  exitCode,
  stdout,
  stderr,
}: {
  command: string[];
  exitCode: number | null;
  stdout: string;
  stderr: string;
}) {
  return (
    <div className={`part-command-execution ${statusClass(exitCode)}`}>
      <div className="part-command-execution__header">
        <code className="part-command-execution__cmd">{command.join(" ")}</code>
        {exitCode !== null && (
          <span className={`part-command-execution__exit ${statusClass(exitCode)}`}>
            exit {exitCode}
          </span>
        )}
      </div>
      {stdout && <pre className="part-command-execution__stdout">{stdout}</pre>}
      {stderr && <pre className="part-command-execution__stderr">{stderr}</pre>}
    </div>
  );
}
