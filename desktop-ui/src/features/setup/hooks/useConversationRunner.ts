import { useCallback, useEffect, useRef, useState } from "react";
import type { ConversationNode, NodeValue, TranscriptValues } from "../schema";
import { CONVERSATION_SCHEMA } from "../schema";

export interface TranscriptEntry {
  node: ConversationNode;
  value: NodeValue;
  status: "completed" | "editing";
}

interface RunnerState {
  transcript: Record<string, TranscriptEntry>;
  activeIndex: number;
  isAnimating: boolean;
  error: string | null;
  isSaving: boolean;
  isLoading: boolean;
}

export function useConversationRunner() {
  const [state, setState] = useState<RunnerState>({
    transcript: {},
    activeIndex: 0,
    isAnimating: true, // Start animating the first prompt
    error: null,
    isSaving: false,
    isLoading: true,
  });

  const schema = CONVERSATION_SCHEMA;
  const stateRef = useRef(state);
  stateRef.current = state;

  // ── Derived values ──────────────────────────────────────────

  const transcriptValues: TranscriptValues = {};
  for (const [id, entry] of Object.entries(state.transcript)) {
    if (entry.status === "completed") {
      transcriptValues[id] = entry.value;
    }
  }

  const activeNode = schema[state.activeIndex] ?? null;
  const completedCount = Object.values(state.transcript).filter(
    (e) => e.status === "completed",
  ).length;
  const totalNodes = schema.filter((n) => n.id !== "complete" && n.id !== "finance_setup").length;
  const progress = totalNodes > 0 ? completedCount / totalNodes : 0;

  // ── Resume: load existing values ────────────────────────────

  useEffect(() => {
    let cancelled = false;

    async function loadExisting() {
      const loaded: Record<string, TranscriptEntry> = {};
      let firstEmpty = 0;

      for (let i = 0; i < schema.length; i++) {
        const node = schema[i];

        // Skip nodes with conditions that depend on not-yet-loaded values
        if (node.condition && !node.condition({})) continue;
        // Skip nodes without loaders
        if (!node.load) {
          firstEmpty = i;
          break;
        }

        try {
          const value = await node.load();
          if (value !== null && value !== undefined && value !== "") {
            loaded[node.id] = { node, value, status: "completed" };
          } else {
            firstEmpty = i;
            break;
          }
        } catch {
          firstEmpty = i;
          break;
        }
      }

      if (cancelled) return;
      setState((prev) => ({
        ...prev,
        transcript: loaded,
        activeIndex: firstEmpty,
        isLoading: false,
        isAnimating: true,
      }));
    }

    loadExisting();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- schema is a module constant, runs once on mount
  }, []);

  // ── Find next valid index (skipping conditions) ─────────────

  const findNextIndex = useCallback((fromIndex: number, values: TranscriptValues): number => {
    for (let i = fromIndex + 1; i < schema.length; i++) {
      const node = schema[i];
      if (!node.condition || node.condition(values)) return i;
    }
    return schema.length; // past the end — all done
  }, []);

  // ── Submit current node ─────────────────────────────────────

  const submit = useCallback(
    async (value: NodeValue) => {
      const { activeIndex, transcript } = stateRef.current;
      const node = schema[activeIndex];
      if (!node) return;

      // Validate
      if (node.validate && typeof value === "string") {
        const err = node.validate(value);
        if (err) {
          setState((prev) => ({ ...prev, error: err }));
          return;
        }
      }

      setState((prev) => ({ ...prev, isSaving: true, error: null }));

      // Build current values including this one
      const currentValues: TranscriptValues = {};
      for (const [id, entry] of Object.entries(transcript)) {
        if (entry.status === "completed") currentValues[id] = entry.value;
      }
      currentValues[node.id] = value;

      // Save
      try {
        if (node.save) {
          await node.save(value, currentValues);
        }
      } catch (e) {
        setState((prev) => ({
          ...prev,
          isSaving: false,
          error: e instanceof Error ? e.message : "Failed to save",
        }));
        return;
      }

      const nextIndex = findNextIndex(activeIndex, currentValues);
      setState((prev) => ({
        ...prev,
        transcript: {
          ...prev.transcript,
          [node.id]: { node, value, status: "completed" },
        },
        activeIndex: nextIndex,
        isSaving: false,
        isAnimating: true,
        error: null,
      }));
    },
    [findNextIndex],
  );

  // ── Re-submit after edit ────────────────────────────────────

  const resubmit = useCallback(async (nodeId: string, value: NodeValue) => {
    const entry = stateRef.current.transcript[nodeId];
    if (!entry) return;

    setState((prev) => ({ ...prev, isSaving: true, error: null }));

    const currentValues: TranscriptValues = {};
    for (const [id, e] of Object.entries(stateRef.current.transcript)) {
      if (e.status === "completed") currentValues[id] = e.value;
    }
    currentValues[nodeId] = value;

    try {
      if (entry.node.save) {
        await entry.node.save(value, currentValues);
      }
    } catch (e) {
      setState((prev) => ({
        ...prev,
        isSaving: false,
        error: e instanceof Error ? e.message : "Failed to save",
      }));
      return;
    }

    setState((prev) => ({
      ...prev,
      transcript: {
        ...prev.transcript,
        [nodeId]: { ...prev.transcript[nodeId], value, status: "completed" },
      },
      isSaving: false,
      error: null,
    }));
  }, []);

  // ── Edit a completed node ───────────────────────────────────

  const startEdit = useCallback((nodeId: string) => {
    setState((prev) => ({
      ...prev,
      transcript: {
        ...prev.transcript,
        [nodeId]: { ...prev.transcript[nodeId], status: "editing" },
      },
      error: null,
    }));
  }, []);

  const cancelEdit = useCallback((nodeId: string) => {
    setState((prev) => ({
      ...prev,
      transcript: {
        ...prev.transcript,
        [nodeId]: { ...prev.transcript[nodeId], status: "completed" },
      },
    }));
  }, []);

  // ── Animation complete ──────────────────────────────────────

  const setAnimationComplete = useCallback(() => {
    setState((prev) => ({ ...prev, isAnimating: false }));
  }, []);

  // ── Finance panel complete ──────────────────────────────────

  const completeFinancePanel = useCallback(() => {
    const currentValues: TranscriptValues = {};
    for (const [id, entry] of Object.entries(stateRef.current.transcript)) {
      if (entry.status === "completed") currentValues[id] = entry.value;
    }
    const financeNode = schema.find((n) => n.id === "finance_setup");
    if (financeNode) {
      const nextIndex = findNextIndex(stateRef.current.activeIndex, currentValues);
      setState((prev) => ({
        ...prev,
        transcript: {
          ...prev.transcript,
          finance_setup: { node: financeNode, value: true, status: "completed" },
        },
        activeIndex: nextIndex,
        isAnimating: true,
      }));
    }
  }, [findNextIndex]);

  return {
    // State
    transcript: state.transcript,
    activeNode,
    activeIndex: state.activeIndex,
    isAnimating: state.isAnimating,
    error: state.error,
    isSaving: state.isSaving,
    isLoading: state.isLoading,
    progress,
    schema,
    transcriptValues,
    // Actions
    submit,
    resubmit,
    startEdit,
    cancelEdit,
    setAnimationComplete,
    completeFinancePanel,
  };
}
