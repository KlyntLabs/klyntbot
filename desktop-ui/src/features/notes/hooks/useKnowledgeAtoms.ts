import { ipc } from "@shared/hooks/useIpc";
import { useMutation } from "@shared/hooks/useMutation";
import { invalidateQueries, useQuery } from "@shared/hooks/useQuery";

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

  const invalidate = () => {
    invalidateQueries("atoms_for_note");
  };

  const accept = async (atomId: string, personalImportance?: number) => {
    await acceptMutate({ atomId, personalImportance } as never);
    invalidate();
  };

  const dismiss = async (atomId: string) => {
    await dismissMutate({ atomId } as never);
    invalidate();
  };

  const restore = async (atomId: string) => {
    await restoreMutate({ atomId } as never);
    invalidate();
  };

  return { accept, dismiss, restore };
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
