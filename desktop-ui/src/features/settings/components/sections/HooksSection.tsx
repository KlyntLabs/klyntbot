import { useEffect, useState } from "react";
import { invoke } from "@/api/client";

interface Snapshot {
  path: string;
  exists: boolean;
  content: string;
}

export function HooksSection() {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  useEffect(() => {
    invoke<Snapshot>("coding_hooks_list").then(setSnap);
  }, []);

  if (!snap) return <div>Loading...</div>;
  if (!snap.exists)
    return (
      <div>
        <p>
          No <code>~/.klyntbot/hooks.toml</code> found.
        </p>
        <p>Hooks are user-managed; create the file to enable.</p>
      </div>
    );
  return (
    <div className="hooks-section">
      <p>
        Hook configuration: <code>{snap.path}</code>{" "}
        <button onClick={() => invoke("open_path", { path: snap.path })}>Open in editor</button>
      </p>
      <pre className="hooks-section__content">{snap.content}</pre>
    </div>
  );
}
