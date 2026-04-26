import type { PerspectiveType } from "../hooks/usePerspective";
import { LinkedViewPanel } from "./LinkedViewPanel";
import { AnnotatedView } from "./perspectives/AnnotatedView";
import { StudyModeView } from "./perspectives/StudyModeView";

interface PerspectiveOverlayProps {
  perspective: PerspectiveType | null;
  noteId: string;
  sectionText: string;
  sectionId: string;
}

export function PerspectiveOverlay({
  perspective,
  noteId,
  sectionText,
  sectionId,
}: PerspectiveOverlayProps) {
  switch (perspective) {
    case "linked-view":
      return <LinkedViewPanel noteId={noteId} sectionText={sectionText} />;
    case "annotated":
      return <AnnotatedView noteId={noteId} sectionId={sectionId} />;
    case "study-mode":
      return <StudyModeView noteId={noteId} sectionId={sectionId} />;
    default:
      return null;
  }
}
