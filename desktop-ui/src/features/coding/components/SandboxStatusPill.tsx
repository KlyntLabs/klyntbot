import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

export function SandboxStatusPill({ threadId }: { threadId: string | null }) {
  const [status, setStatus] = useState<"idle" | "macos" | "linux" | "unsandboxed">("idle");
  const [policySummary, setPolicySummary] = useState("");

  useEffect(() => {
    if (!threadId) return;
    let active = true;
    let unlisten: (() => void) | undefined;
    (async () => {
      unlisten = await listen<{
        thread_id: string;
        runner: string;
        policy_summary: string;
        fallback_unsandboxed: boolean;
      }>("agent:sandbox_policy_applied", (e) => {
        if (e.payload.thread_id !== threadId) return;
        if (e.payload.fallback_unsandboxed) setStatus("unsandboxed");
        else if (e.payload.runner === "macos") setStatus("macos");
        else if (e.payload.runner === "linux") setStatus("linux");
        setPolicySummary(e.payload.policy_summary);
      });
      if (!active && unlisten) {
        unlisten();
        unlisten = undefined;
      }
    })();
    return () => {
      active = false;
      unlisten?.();
    };
  }, [threadId]);

  const label = {
    idle: "⌛ idle",
    macos: "🔒 macOS",
    linux: "🔒 Linux",
    unsandboxed: "⚠ unsandboxed",
  }[status];

  return (
    <span className={`sandbox-pill sandbox-pill--${status}`} title={policySummary}>
      {label}
    </span>
  );
}
