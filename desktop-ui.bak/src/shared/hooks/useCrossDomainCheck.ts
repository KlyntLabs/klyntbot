import { ipc } from "@shared/hooks/useIpc";
import { useEffect } from "react";

/**
 * Fire-and-forget cross-domain connection check.
 * Call when a task/note detail view mounts to trigger BrainVoice ambient signals.
 */
export function useCrossDomainCheck(
  domain: "task" | "note" | null,
  id: string | undefined,
  title: string | undefined,
  createdAt?: string | null,
) {
  useEffect(() => {
    if (!domain || !id || !title || title.length < 10) return;

    // Small delay to ensure embeddings are indexed
    const timer = setTimeout(() => {
      ipc("cross_domain_check", { domain, id, title, createdAt }).catch(() => {});
    }, 500);

    return () => clearTimeout(timer);
  }, [domain, id, title, createdAt]);
}
