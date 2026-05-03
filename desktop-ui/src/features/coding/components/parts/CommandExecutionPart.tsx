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
    <div className={`part-command-execution ${exitCode === 0 ? "success" : exitCode !== null ? "error" : "running"}`}>
      <div className="part-command-execution__header">
        <code className="part-command-execution__cmd">{command.join(" ")}</code>
        {exitCode !== null && (
          <span className={`part-command-execution__exit ${exitCode === 0 ? "success" : "error"}`}>
            exit {exitCode}
          </span>
        )}
      </div>
      {stdout && <pre className="part-command-execution__stdout">{stdout}</pre>}
      {stderr && <pre className="part-command-execution__stderr">{stderr}</pre>}
    </div>
  );
}
