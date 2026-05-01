import { useEffect, useState } from "react";
import { commands } from "@/bindings";

export function HooksSubsection() {
  const [snapshot, setSnapshot] = useState<{ path: string; exists: boolean; content: string } | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    commands
      .codingHooksList()
      .then((res) => {
        if (res.status === "ok") {
          setSnapshot(res.data);
        } else {
          setError("Failed to load hooks.");
        }
      })
      .catch((e: unknown) => setError(String(e)));
  }, []);

  if (error) return <div className="settings-error">{error}</div>;
  if (!snapshot) return <div className="settings-empty">Loading hooks…</div>;
  if (!snapshot.exists)
    return (
      <div className="settings-empty">
        No hooks configured. Add some in <code>{snapshot.path}</code>.
      </div>
    );

  return (
    <div className="hooks-subsection">
      <p className="hooks-subsection__path">
        Reading from <code>{snapshot.path}</code>
      </p>
      <pre className="hooks-subsection__toml">{snapshot.content}</pre>
    </div>
  );
}
