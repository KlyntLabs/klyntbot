import { ActiveReviewSession } from "../review/ActiveReviewSession";

export function FlashcardReview({ onClose }: { onClose: () => void }) {
  return <ActiveReviewSession layout="compact" onClose={onClose} />;
}
