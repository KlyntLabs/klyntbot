import { ThinkingDots } from "@shared/ui/ThinkingDots";
import { ArrowLeft, Edit3, ExternalLink, Lightbulb } from "lucide-react";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router";
import { useReviewSession } from "../hooks/useReviewSession";
import { CardEditor } from "./CardEditor";
import { CardRenderer } from "./CardRenderer";
import { PostSession } from "./PostSession";
import { RatingButtons } from "./RatingButtons";

interface ImmersiveReviewProps {
  deck?: string;
  onExit: () => void;
}

export function ImmersiveReview({ deck, onExit }: ImmersiveReviewProps) {
  const [loading, setLoading] = useState(true);
  const [editing, setEditing] = useState(false);
  const navigate = useNavigate();

  const session = useReviewSession();
  const {
    current,
    revealed,
    reveal,
    rate,
    cards,
    currentIndex,
    updateCard,
    done: sessionDone,
    socraticExplanation,
    socraticLoading,
    showSocratic,
    dismissSocratic,
  } = session;

  const [socraticOpen, setSocraticOpen] = useState(false);

  // Start review on mount — intentionally excludes session.startReview to avoid re-triggering
  // biome-ignore lint/correctness/useExhaustiveDependencies: only run on mount/deck change
  useEffect(() => {
    session.startReview(deck).then(() => setLoading(false));
  }, [deck]);

  // Keyboard shortcuts
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // Don't capture keys if an input is focused
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA") return;

      if (e.key === "Escape") {
        e.preventDefault();
        onExit();
        return;
      }

      if (e.key === " " && !revealed && current) {
        e.preventDefault();
        reveal();
        return;
      }

      if ((e.key === "e" || e.key === "E") && revealed && current && !editing) {
        e.preventDefault();
        setEditing(true);
        return;
      }

      if (revealed && current) {
        const ratingMap: Record<string, "again" | "hard" | "good" | "easy"> = {
          "1": "again",
          "2": "hard",
          "3": "good",
          "4": "easy",
        };
        const quality = ratingMap[e.key];
        if (quality) {
          e.preventDefault();
          rate(quality);
        }

        if (e.key === "s" || e.key === "S") {
          if (current?.sourceNoteId) {
            e.preventDefault();
            navigate(`/notes?id=${current.sourceNoteId}`);
          }
          return;
        }
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [revealed, current, reveal, rate, onExit, navigate, editing]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: reset state when card advances
  useEffect(() => {
    setSocraticOpen(false);
    setEditing(false);
  }, [currentIndex]);

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <ThinkingDots />
      </div>
    );
  }

  // No cards available
  if (cards.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center space-y-3">
          <p className="text-muted-foreground text-sm">No cards due for review</p>
          <button
            type="button"
            onClick={onExit}
            className="glass-button px-4 py-2 text-sm text-foreground"
          >
            Back to Dashboard
          </button>
        </div>
      </div>
    );
  }

  // Post-session summary
  if (sessionDone) {
    return (
      <PostSession
        totalReviewed={session.totalReviewed}
        correctCount={session.correctCount}
        elapsedSeconds={session.elapsedSeconds}
        onBackToDashboard={onExit}
      />
    );
  }

  const progress = cards.length > 0 ? (currentIndex / cards.length) * 100 : 0;

  return (
    <div className="flex-1 flex flex-col">
      {/* Top bar */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-border">
        <button
          type="button"
          onClick={onExit}
          className="flex items-center gap-1.5 text-sm text-muted-foreground hover:text-foreground transition-colors"
        >
          <ArrowLeft size={16} strokeWidth={1.5} />
          <span className="text-xs">ESC</span>
        </button>

        <span className="text-xs text-muted-foreground tabular-nums">
          {currentIndex + 1} / {cards.length}
        </span>

        <span className="text-xs text-muted-foreground truncate max-w-[120px]">
          {current?.deck ?? deck ?? "All decks"}
        </span>
      </div>

      {/* Card area */}
      <div className="flex-1 flex flex-col items-center justify-center px-6 py-8">
        {current && (
          <div className="w-full max-w-lg">
            {editing ? (
              <CardEditor
                card={current}
                onSaved={(updated) => {
                  updateCard(currentIndex, updated);
                  setEditing(false);
                }}
                onCancel={() => setEditing(false)}
              />
            ) : (
              <div className="glass-card p-8">
                <CardRenderer card={current} revealed={revealed} />
              </div>
            )}
          </div>
        )}
      </div>

      {/* Socratic explanation */}
      {showSocratic && (socraticLoading || socraticExplanation) && (
        <div className="px-6 animate-[fade-in-up_0.2s_ease-out]">
          {socraticLoading ? (
            <div className="flex items-center gap-2 text-xs text-muted-foreground justify-center py-2">
              <Lightbulb size={14} strokeWidth={1.5} className="animate-pulse" />
              Thinking...
            </div>
          ) : socraticExplanation && !socraticOpen ? (
            <button
              type="button"
              onClick={() => setSocraticOpen(true)}
              className="mx-auto flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors py-2"
            >
              <Lightbulb size={14} strokeWidth={1.5} />
              Let's understand why
            </button>
          ) : socraticExplanation ? (
            <div className="glass-card p-4 text-sm text-foreground whitespace-pre-wrap animate-[fade-in-up_0.2s_ease-out]">
              {socraticExplanation}
              <button
                type="button"
                onClick={dismissSocratic}
                className="block mt-2 text-xs text-muted-foreground hover:text-foreground"
              >
                Dismiss
              </button>
            </div>
          ) : null}
        </div>
      )}

      {/* Bottom area: Show Answer or Rating buttons */}
      <div className="px-6 pb-4 space-y-3">
        {!revealed ? (
          <div className="flex justify-center">
            <button
              type="button"
              onClick={reveal}
              className="glass-button px-8 py-2.5 text-sm text-foreground"
            >
              Show Answer
              <span className="text-2xs text-muted-foreground ml-2">Space</span>
            </button>
          </div>
        ) : (
          <RatingButtons onRate={rate} />
        )}

        {/* Footer actions */}
        <div className="flex items-center justify-center gap-4">
          <button
            type="button"
            onClick={() => setEditing(true)}
            disabled={!revealed || !current}
            className={`flex items-center gap-1 text-[11px] transition-colors ${
              revealed && current
                ? "text-muted-foreground hover:text-foreground cursor-pointer"
                : "text-muted-foreground opacity-50 cursor-not-allowed"
            }`}
          >
            <Edit3 size={12} strokeWidth={1.5} />
            Edit
            <span className="text-2xs text-muted-foreground ml-0.5">E</span>
          </button>
          <button
            type="button"
            onClick={() => {
              if (current?.sourceNoteId) {
                navigate(`/notes?id=${current.sourceNoteId}`);
              }
            }}
            disabled={!current?.sourceNoteId}
            className={`flex items-center gap-1 text-[11px] transition-colors ${
              current?.sourceNoteId
                ? "text-muted-foreground hover:text-foreground cursor-pointer"
                : "text-muted-foreground opacity-50 cursor-not-allowed"
            }`}
          >
            <ExternalLink size={12} strokeWidth={1.5} />
            Source
          </button>
        </div>
      </div>

      {/* Progress bar */}
      <div className="h-1 bg-white/[0.04]">
        <div
          className="h-full bg-brand transition-all duration-300 ease-out"
          style={{ width: `${progress}%` }}
        />
      </div>
    </div>
  );
}
