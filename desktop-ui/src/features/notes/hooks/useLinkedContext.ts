import { useQuery } from "@shared/hooks/useQuery";

export interface LinkedContextResponse {
  semanticFacts: LinkedFact[];
  episodicMemories: LinkedMemory[];
  relatedAnnotations: LinkedAnnotation[];
  proceduralRules: LinkedRule[];
}

export interface LinkedFact {
  id: string;
  subject: string;
  predicate: string;
  object: string;
  confidence: number;
  sourceNote: string | null;
}

export interface LinkedMemory {
  id: string;
  content: string;
  domain: string;
  createdAt: string;
}

export interface LinkedAnnotation {
  id: string;
  noteId: string;
  markId: string | null;
  content: string;
  quotedText: string | null;
  tags: string;
  createdAt: string;
  updatedAt: string;
}

export interface LinkedRule {
  id: string;
  ruleText: string;
  domain: string;
  signalCount: number;
}

const EMPTY_CONTEXT: LinkedContextResponse = {
  semanticFacts: [],
  episodicMemories: [],
  relatedAnnotations: [],
  proceduralRules: [],
};

export function useLinkedContext(noteId: string | null, sectionText: string | null) {
  const enabled = !!noteId && !!sectionText && sectionText.length > 10;
  const { data, loading, error } = useQuery<LinkedContextResponse>(
    "note_get_linked_context",
    enabled ? { params: { noteId, sectionText: sectionText.slice(0, 500) } } : null,
    EMPTY_CONTEXT,
    10 * 60 * 1000, // 10 minutes staleTime
  );

  return { context: data ?? EMPTY_CONTEXT, loading, error };
}
