import { ipc } from "@shared/hooks/useIpc";
import { BookOpen, X } from "lucide-react";
import { useCallback, useEffect } from "react";
import { useActiveReview } from "../../hooks/useActiveReview";
import type { DeckSummary, ReviewQuality } from "../../hooks/useFlashcards";
import { ModeSelector } from "./ModeSelector";
import { ReviewCard } from "./ReviewCard";
import { SessionProgress } from "./SessionProgress";
import { SessionSummary } from "./SessionSummary";

interface ActiveReviewSessionProps {
  layout: "compact" | "fullscreen";
  onClose: () => void;
}

export function ActiveReviewSession({ layout: _layout, onClose }: ActiveReviewSessionProps) {
  const {
    phase,
    cardPhase,
    decks,
    current,
    remaining,
    gradeResult,
    lastAnswer,
    selectedMode,
    selectedDeck,
    avgScore,
    stats,
    error,
    fetchDecks,
    startReview,
    submitAnswer,
    confirmRating,
    requestExplanation,
    switchMode,
    skipCard,
    saveSession,
  } = useActiveReview();

  const handleExit = useCallback(() => {
    if (phase === "reviewing" && stats.current.cardsReviewed > 0) {
      saveSession("abandoned");
    }
    onClose();
  }, [phase, saveSession, onClose, stats]);

  const handleSaveInsight = useCallback(async () => {
    const s = stats.current;
    const avgPct = s.cardsReviewed > 0 ? Math.round((s.totalScore / s.cardsReviewed) * 100) : 0;
    const weakList = s.weakCards
      .map((w) => `- ${w.front} (${Math.round(w.score * 100)}%)`)
      .join("\n");
    const body = `# Review Session\n\n**Score:** ${avgPct}%\n**Cards:** ${s.cardsReviewed}\n\n${weakList ? `## Weak spots\n${weakList}` : "All cards held strong."}`;

    try {
      await ipc("note_create", { title: `Review ${new Date().toLocaleDateString()}`, body });
    } catch {
      // best-effort
    }
  }, [stats]);

  const handleJumpToSource = useCallback(() => {
    if (current?.sourceNoteId) {
      window.dispatchEvent(
        new CustomEvent("navigate-to-note", { detail: { noteId: current.sourceNoteId } }),
      );
      handleExit();
    }
  }, [current, handleExit]);

  const handleReviewWeak = useCallback(() => {
    if (selectedDeck) {
      startReview(selectedDeck);
    }
  }, [selectedDeck, startReview]);

  // Fetch decks on mount
  useEffect(() => {
    fetchDecks();
  }, [fetchDecks]);

  // Keyboard shortcuts
  useEffect(() => {
    if (phase !== "reviewing") return;

    const handleKeyDown = (e: KeyboardEvent) => {
      // Don't fire shortcuts while typing in an input/textarea
      const target = e.target as HTMLElement;
      if (target.tagName === "TEXTAREA" || target.tagName === "INPUT") return;

      switch (e.key) {
        case "Enter":
          if (cardPhase === "graded" || cardPhase === "socratic") {
            confirmRating();
          }
          break;
        case "1":
          if (cardPhase === "graded" || cardPhase === "socratic") {
            confirmRating("again");
          }
          break;
        case "2":
          if (cardPhase === "graded" || cardPhase === "socratic") {
            confirmRating("hard");
          }
          break;
        case "3":
          if (cardPhase === "graded" || cardPhase === "socratic") {
            confirmRating("good");
          }
          break;
        case "4":
          if (cardPhase === "graded" || cardPhase === "socratic") {
            confirmRating("easy");
          }
          break;
        case "e":
          if (cardPhase === "graded") {
            requestExplanation();
          }
          break;
        case "s":
          if (cardPhase === "graded" || cardPhase === "socratic") {
            handleSaveInsight();
          }
          break;
        case "j":
          if (cardPhase === "graded" || cardPhase === "socratic") {
            handleJumpToSource();
          }
          break;
        case "Tab":
          e.preventDefault();
          switchMode(selectedMode === "self_grade" ? "typed" : "self_grade");
          break;
        case "Escape":
          handleExit();
          break;
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [
    phase,
    cardPhase,
    confirmRating,
    requestExplanation,
    handleExit,
    switchMode,
    selectedMode,
    handleSaveInsight,
    handleJumpToSource,
  ]);

  // ── Idle — initial loading state ─────────────────────────────────────────

  if (phase === "idle") {
    return (
      <div className="flex flex-col items-center justify-center gap-3 py-8">
        <p className="text-[11px] text-dim">Loading decks…</p>
      </div>
    );
  }

  // ── Deck picker ───────────────────────────────────────────────────────────

  if (phase === "deck_picker") {
    const dueDecks = decks.filter((d: DeckSummary) => d.dueCount > 0);

    if (dueDecks.length === 0) {
      return (
        <div className="flex flex-col items-center justify-center gap-3 py-8">
          <BookOpen size={24} className="text-accent" />
          <p className="text-xs text-foreground font-medium">No cards due for review</p>
          <p className="text-2xs text-dim">
            {decks.length} {decks.length === 1 ? "deck" : "decks"} saved, all caught up!
          </p>
          <button
            type="button"
            onClick={onClose}
            className="text-2xs px-3 py-1 rounded-md bg-white/[0.06] text-muted-foreground hover:text-foreground"
          >
            Done
          </button>
        </div>
      );
    }

    return (
      <div className="flex flex-col gap-3 p-3">
        <div className="flex items-center gap-2">
          <span className="text-[11px] text-foreground font-medium">Choose a deck to review</span>
          <div className="flex-1" />
          <button type="button" onClick={handleExit} className="p-1 text-dim hover:text-foreground">
            <X size={12} />
          </button>
        </div>

        {error && (
          <p className="text-2xs text-red-400 bg-red-500/10 rounded-md px-2 py-1">{error}</p>
        )}

        <div className="space-y-1.5">
          {dueDecks.map((d: DeckSummary) => (
            <button
              key={d.name}
              type="button"
              onClick={() => startReview(d.name)}
              className="w-full flex items-center gap-2 p-2 rounded-lg bg-white/[0.03] hover:bg-white/[0.06] text-left"
            >
              <BookOpen size={12} className="text-muted-foreground shrink-0" />
              <span className="text-[11px] text-foreground truncate flex-1">{d.name}</span>
              <span className="text-2xs text-dim shrink-0">
                {d.dueCount}/{d.cardCount} due
              </span>
            </button>
          ))}
        </div>
      </div>
    );
  }

  // ── Complete ──────────────────────────────────────────────────────────────

  if (phase === "complete") {
    return (
      <SessionSummary
        stats={stats.current}
        onClose={onClose}
        onSaveInsight={handleSaveInsight}
        onReviewWeak={handleReviewWeak}
        onSaveReflection={async (text) => {
          try {
            await ipc("note_create", {
              title: `Reflection ${new Date().toLocaleDateString()}`,
              body: text,
            });
          } catch {
            // best-effort
          }
        }}
      />
    );
  }

  // ── Reviewing ─────────────────────────────────────────────────────────────

  if (!current) return null;

  const total = remaining + (cardPhase !== "answering" ? 1 : 0);

  return (
    <div className="flex flex-col gap-3 p-3">
      {/* Progress */}
      <SessionProgress
        remaining={remaining}
        total={total}
        avgScore={avgScore}
        onExit={handleExit}
      />

      {/* Error */}
      {error && <p className="text-2xs text-red-400 bg-red-500/10 rounded-md px-2 py-1">{error}</p>}

      {/* Card */}
      <ReviewCard
        card={current}
        cardPhase={cardPhase}
        mode={selectedMode}
        gradeResult={gradeResult}
        lastAnswer={lastAnswer}
        propagationCount={stats.current.propagationCount}
        onSubmitAnswer={submitAnswer}
        onConfirmRating={confirmRating}
        onExplain={requestExplanation}
        onSaveInsight={handleSaveInsight}
        onJumpToSource={handleJumpToSource}
        onSelfRate={(quality: ReviewQuality) => {
          confirmRating(quality);
        }}
      />

      {/* Mode selector + skip */}
      <div className="flex items-center justify-between gap-2">
        <ModeSelector current={selectedMode} onChange={switchMode} />
        <button
          type="button"
          onClick={skipCard}
          className="shrink-0 text-[9px] text-dim hover:text-foreground"
        >
          Skip
        </button>
      </div>
    </div>
  );
}
