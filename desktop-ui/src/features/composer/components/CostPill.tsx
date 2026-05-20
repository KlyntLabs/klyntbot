/// Extracted from `features/coding/components/CostPill.tsx` during the
/// unify-to-assistant refactor.
///
/// NOTE: Cost tracking currently has no assistant-mode backend endpoint.
/// The component is preserved for future wiring to a unified cost API.
/// Until then it renders nothing.

export function CostPill({ threadId: _threadId }: { threadId: string | null }) {
  // TODO: Wire to unified assistant cost endpoint once available.
  // Original implementation used `coding_thread_read` + `agent:cost_update`.
  return null;
}
