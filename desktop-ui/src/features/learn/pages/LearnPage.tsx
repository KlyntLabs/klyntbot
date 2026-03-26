import { invalidateQueries } from "@shared/hooks/useQuery";
import { useCallback, useEffect, useState } from "react";
import { useSearchParams } from "react-router";
import { CardGenerationModal } from "../../notes/components/CardGenerationModal";
import { useCardGeneration } from "../../notes/hooks/useCardGeneration";
import { DashboardHome } from "../components/DashboardHome";
import { ImmersiveReview } from "../components/ImmersiveReview";
import { QuickAdd } from "../components/QuickAdd";

type ViewMode = "dashboard" | "review";

export default function LearnPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const [mode, setMode] = useState<ViewMode>("dashboard");
  const [reviewDeck, setReviewDeck] = useState<string | undefined>();
  const [quickAddOpen, setQuickAddOpen] = useState(false);
  const cardGen = useCardGeneration();
  const [cardGenOpen, setCardGenOpen] = useState(false);
  const { generateFromNote, generateFromText } = cardGen;

  // Enter review mode when navigated with ?review=true (e.g. from tray)
  useEffect(() => {
    if (searchParams.get("review") === "true") {
      setMode("review");
      setSearchParams({}, { replace: true });
    }
  }, [searchParams, setSearchParams]);

  const handleGenerateFromNote = useCallback(
    (noteId: string) => {
      setCardGenOpen(true);
      generateFromNote(noteId);
    },
    [generateFromNote],
  );

  const handleGenerateFromText = useCallback(
    (text: string) => {
      setCardGenOpen(true);
      generateFromText(text);
    },
    [generateFromText],
  );

  const handleStartReview = useCallback((deck?: string) => {
    setReviewDeck(deck);
    setMode("review");
  }, []);

  const handleExitReview = useCallback(() => {
    setMode("dashboard");
    setReviewDeck(undefined);
    invalidateQueries("flashcard_");
  }, []);

  // Cmd+N / Ctrl+N shortcut for Quick Add
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "n" && mode === "dashboard") {
        e.preventDefault();
        setQuickAddOpen(true);
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [mode]);

  if (mode === "review") {
    return <ImmersiveReview deck={reviewDeck} onExit={handleExitReview} />;
  }

  return (
    <>
      <DashboardHome
        onStartReview={handleStartReview}
        onQuickAdd={() => setQuickAddOpen(true)}
        onGenerateFromNote={handleGenerateFromNote}
        onGenerateFromText={handleGenerateFromText}
        generating={cardGen.generating}
      />
      <QuickAdd
        open={quickAddOpen}
        onClose={() => setQuickAddOpen(false)}
        onCreated={() => {
          setQuickAddOpen(false);
          invalidateQueries("flashcard_");
          invalidateQueries("review_stats_");
        }}
      />
      <CardGenerationModal
        open={cardGenOpen}
        generating={cardGen.generating}
        previews={cardGen.previews}
        deckSuggestion={cardGen.deckSuggestion}
        approved={cardGen.approved}
        error={cardGen.error}
        saving={cardGen.saving}
        onToggleCard={cardGen.toggleCard}
        onEditCard={cardGen.editCard}
        onSave={(noteId, deck) => {
          cardGen.saveApproved(noteId, deck).then(() => setCardGenOpen(false));
        }}
        onClose={() => {
          cardGen.reset();
          setCardGenOpen(false);
        }}
        noteId={null}
      />
    </>
  );
}
