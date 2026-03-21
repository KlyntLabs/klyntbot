import { ipc } from "@shared/hooks/useIpc";
import { useMutation } from "@shared/hooks/useMutation";
import { invalidateQueries, useQuery } from "@shared/hooks/useQuery";
import { useCallback } from "react";

export interface KnowledgeAtomResponse {
  id: string;
  subject: string;
  atomType: string;
  domain: string;
  sourceNoteId: string | null;
  sourceRange: string | null;
  sourceContext: string | null;
  retentionPct: number;
  personalImportance: number;
  status: string;
  salience: number;
  lastInteractionTs: string | null;
  metadata: string | null;
  topicName: string | null;
  linkedCardCount: number;
  createdAt: string;
}

export function useKnowledgeAtoms(noteId: string | null) {
  const { data, loading, refetch } = useQuery<KnowledgeAtomResponse[]>(
    "atoms_for_note",
    noteId ? { params: { noteId } } : null,
    [],
  );

  const activeAtoms = data?.filter((a) => a.status === "active") ?? [];
  const suggestedAtoms = data?.filter((a) => a.status === "suggested") ?? [];

  return { activeAtoms, suggestedAtoms, loading, refetch };
}

export function useAtomAccept() {
  return useMutation<KnowledgeAtomResponse>("atom_accept", "params");
}

export function useAtomDismiss() {
  return useMutation<void>("atom_dismiss", "params");
}

export function useAtomRestore() {
  return useMutation<KnowledgeAtomResponse>("atom_restore", "params");
}

export function useAtomActions(noteId: string | null) {
  const { mutate: acceptMutate } = useAtomAccept();
  const { mutate: dismissMutate } = useAtomDismiss();
  const { mutate: restoreMutate } = useAtomRestore();

  const invalidate = useCallback(() => {
    invalidateQueries("atoms_for_note");
  }, []);

  const accept = useCallback(
    async (atomId: string, personalImportance?: number) => {
      await acceptMutate({ atomId, personalImportance } as never);
      invalidate();
    },
    [acceptMutate, invalidate],
  );

  const dismiss = useCallback(
    async (atomId: string) => {
      await dismissMutate({ atomId } as never);
      invalidate();
    },
    [dismissMutate, invalidate],
  );

  const restore = useCallback(
    async (atomId: string) => {
      await restoreMutate({ atomId } as never);
      invalidate();
    },
    [restoreMutate, invalidate],
  );

  const acceptAll = useCallback(
    async (atoms: KnowledgeAtomResponse[]) => {
      for (const atom of atoms) {
        await acceptMutate({ atomId: atom.id } as never);
      }
      invalidate();
    },
    [acceptMutate, invalidate],
  );

  return { accept, dismiss, restore, acceptAll };
}

export async function fetchNextCard(atomId: string) {
  return ipc<FlashcardForReview | null>("atom_next_card", { params: { atomId } });
}

export interface FlashcardForReview {
  id: string;
  front: string;
  back: string;
  cardType: string;
  vocabData: Record<string, string> | null;
  stability: number;
  difficulty: number;
  state: string;
  reviewCount: number;
}
