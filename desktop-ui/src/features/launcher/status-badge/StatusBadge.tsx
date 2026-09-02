import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

type Kind = "success" | "warn" | "error" | "info";
interface Payload {
  text: string;
  kind: Kind;
  ms: number;
}

const COLORS: Record<Kind, string> = {
  success: "text-green-400",
  warn: "text-yellow-400",
  error: "text-red-400",
  info: "text-fg",
};

export function StatusBadge() {
  const [payload, setPayload] = useState<Payload | null>(null);

  useEffect(() => {
    const u1 = listen<Payload>("badge:show", (e) => setPayload(e.payload));
    const u2 = listen<Payload>("badge:update", (e) => setPayload(e.payload));
    return () => {
      u1.then((f) => f());
      u2.then((f) => f());
    };
  }, []);

  if (!payload) return null;
  return (
    <div className="glass-panel h-full w-full rounded-md flex items-center px-3 text-sm">
      <span className={`${COLORS[payload.kind]} truncate`}>{payload.text}</span>
    </div>
  );
}
