import { invalidateQueries } from "@shared/hooks/useQuery";
import { useCallback, useEffect, useState } from "react";
import { DashboardHome } from "../components/DashboardHome";
import { ImmersiveReview } from "../components/ImmersiveReview";
import { QuickAdd } from "../components/QuickAdd";

type ViewMode = "dashboard" | "review";

export default function LearnPage() {
  const [mode, setMode] = useState<ViewMode>("dashboard");
  const [reviewDeck, setReviewDeck] = useState<string | undefined>();
  const [quickAddOpen, setQuickAddOpen] = useState(false);

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
      <DashboardHome onStartReview={handleStartReview} onQuickAdd={() => setQuickAddOpen(true)} />
      <QuickAdd
        open={quickAddOpen}
        onClose={() => setQuickAddOpen(false)}
        onCreated={() => {
          setQuickAddOpen(false);
          invalidateQueries("flashcard_");
        }}
      />
    </>
  );
}
