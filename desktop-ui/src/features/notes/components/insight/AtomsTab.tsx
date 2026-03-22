import { KnowledgeAtomsPanel } from "../KnowledgeAtomsPanel";

interface AtomsTabProps {
  noteId: string | null;
}

export function AtomsTab({ noteId }: AtomsTabProps) {
  return <KnowledgeAtomsPanel noteId={noteId} variant="panel" />;
}
