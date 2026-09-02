import { ThinkingDots } from "@shared/ui/ThinkingDots";
import { ArrowLeft, Edit3, ExternalLink, Keyboard, Lightbulb, RotateCcw } from "lucide-react";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router";
import { useReviewSession } from "../hooks/useReviewSession";
import { AnswerInput } from "./AnswerInput";
import { CardEditor } from "./CardEditor";
import { CardRenderer } from "./CardRenderer";
import { GradeDisplay } from "./GradeDisplay";
import { PostSession } from "./PostSession";
import { RatingButtons } from "./RatingButtons";

interface ImmersiveReviewProps {
  deck?: string;
  onExit: () => void;
}

function ProgressSegments({ total, current }: { total: number; current: number }) {
  if (total <= 1) return null;

  // For many cards, show a continuous bar instead of segments
  if (total > 20) {
    const pct = Math.round((current / total) * 100);
    return (
      <div className="flex items-center gap-2.5 min-w-0">
        <div className="flex-1 h-1 rounded-full bg-white/[0.06] overflow-hidden">
          <div
            className="h-full rounded-full bg-brand transition-all duration-500 ease-out"
            style={{ width: `${pct}%` }}
          />
        </div>
        <span className="text-[10px] text-fg-dim tabular-nums shrink-0">{pct}%</span>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-1">
      {Array.from({ length: total }, (_, i) => (
        <div
          key={i}
          className={`h-1 rounded-full transition-all duration-300 ease-out ${
            i < current
              ? "bg-brand w-3"
              : i === current
                ? "bg-foreground w-5"
                : "bg-white/[0.08] w-3"
          }`}
        />
      ))}
    </div>
  );
}

export function ImmersiveReview({ deck, onExit }: ImmersiveReviewProps) {
  const [loading, setLoading] = useState(true);
  const [editing, setEditing] = useState(false);
  const [answerMode, setAnswerMode] = useState<"flip" | "type">("flip");
  const [typedAnswer, setTypedAnswer] = useState("");
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
    submitAnswer,
    gradeResult,
    grading,
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
      // Tab to toggle mode — must be checked before the input guard
      // so it works even when the type-mode textarea is focused
      if (e.key === "Tab" && !revealed) {
        e.preventDefault();
        setAnswerMode((m) => (m === "flip" ? "type" : "flip"));
        return;
      }

      // Don't capture other keys if an input is focused
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA") return;

      if (e.key === "Escape") {
        e.preventDefault();
        onExit();
        return;
      }

      if (e.key === " " && !revealed && current && answerMode === "flip") {
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
  }, [revealed, current, reveal, rate, onExit, navigate, editing, answerMode]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: reset state when card advances
  useEffect(() => {
    setSocraticOpen(false);
    setEditing(false);
    setTypedAnswer("");
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
          <p className="text-fg-secondary text-sm">No cards due for review</p>
          <button
            type="button"
            onClick={onExit}
            className="glass-button px-4 py-2 text-sm text-fg"
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

  const deckLabel = current?.deck ?? deck ?? "All decks";

  return (
    <div className="flex-1 flex flex-col">
      {/* ── Top bar ──────────────────────────────────────────────── */}
      <div className="flex items-center gap-3 px-4 py-3 border-b border-separator">
        {/* Left: back */}
        <button
          type="button"
          onClick={onExit}
          className="flex items-center gap-1.5 text-fg-secondary hover:text-fg transition-colors shrink-0"
        >
          <ArrowLeft size={15} strokeWidth={1.5} />
          <span className="text-ui-xs hidden sm:inline">ESC</span>
        </button>

        {/* Center: progress + counter */}
        <div className="flex-1 flex flex-col items-center gap-1 min-w-0">
          <ProgressSegments total={cards.length} current={currentIndex} />
          <div className="flex items-center gap-1.5">
            <span className="text-ui-xs text-fg-secondary truncate max-w-[140px]">
              {deckLabel}
            </span>
            <span className="text-fg-dim">·</span>
            <span className="text-ui-xs text-fg-dim tabular-nums">
              {currentIndex + 1}/{cards.length}
            </span>
          </div>
        </div>

        {/* Right: mode toggle (Tab to switch) */}
        <div className="flex items-center gap-1 shrink-0" title="Tab to switch">
          <button
            type="button"
            onClick={() => setAnswerMode("flip")}
            className={`flex items-center gap-1 px-2 py-1 rounded text-ui-xs transition-colors ${
              answerMode === "flip"
                ? "bg-white/10 text-fg"
                : "text-fg-secondary hover:text-fg"
            }`}
          >
            <RotateCcw size={11} strokeWidth={1.5} />
            Flip
          </button>
          <button
            type="button"
            onClick={() => setAnswerMode("type")}
            className={`flex items-center gap-1 px-2 py-1 rounded text-ui-xs transition-colors ${
              answerMode === "type"
                ? "bg-white/10 text-fg"
                : "text-fg-secondary hover:text-fg"
            }`}
          >
            <Keyboard size={11} strokeWidth={1.5} />
            Type
          </button>
          <span className="text-[9px] text-fg-dim ml-0.5">Tab</span>
        </div>
      </div>

      {/* ── Card area ────────────────────────────────────────────── */}
      <div className="flex-1 flex flex-col items-center justify-center px-6 py-6 min-h-0">
        {current && (
          <div className="w-full max-w-xl animate-[fade-in-up_0.2s_ease-out]">
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
              <div className="island p-8 sm:p-10 relative">
                {/* Subtle ambient glow behind the card */}
                <div className="absolute inset-0 -z-10 rounded-[inherit] bg-white/[0.02] blur-xl scale-105" />
                <CardRenderer card={current} revealed={revealed} />
              </div>
            )}
          </div>
        )}
      </div>

      {/* ── Socratic explanation ──────────────────────────────────── */}
      {showSocratic && (socraticLoading || socraticExplanation) && (
        <div className="px-6 max-w-xl mx-auto w-full animate-[fade-in-up_0.2s_ease-out]">
          {socraticLoading ? (
            <div className="flex items-center gap-2 text-ui-sm text-fg-secondary justify-center py-2">
              <Lightbulb size={14} strokeWidth={1.5} className="animate-pulse" />
              Thinking...
            </div>
          ) : socraticExplanation && !socraticOpen ? (
            <button
              type="button"
              onClick={() => setSocraticOpen(true)}
              className="mx-auto flex items-center gap-1.5 text-ui-sm text-fg-secondary hover:text-fg transition-colors py-2"
            >
              <Lightbulb size={14} strokeWidth={1.5} />
              Let's understand why
            </button>
          ) : socraticExplanation ? (
            <div className="island p-4 text-sm text-fg whitespace-pre-wrap animate-[fade-in-up_0.2s_ease-out]">
              {socraticExplanation}
              <button
                type="button"
                onClick={dismissSocratic}
                className="block mt-2 text-ui-sm text-fg-secondary hover:text-fg"
              >
                Dismiss
              </button>
            </div>
          ) : null}
        </div>
      )}

      {/* ── Grade display (typed mode) ───────────────────────────── */}
      {answerMode === "type" && gradeResult && revealed && (
        <div className="px-6 animate-[fade-in-up_0.2s_ease-out]">
          <GradeDisplay result={gradeResult} userAnswer={typedAnswer} />
        </div>
      )}

      {/* ── Bottom controls ──────────────────────────────────────── */}
      <div className="px-6 pb-5 pt-2 space-y-3">
        {answerMode === "flip" ? (
          !revealed ? (
            <div className="flex justify-center">
              <button
                type="button"
                onClick={reveal}
                className="glass-button px-10 py-3 text-sm text-fg font-medium group"
              >
                Show Answer
                <span className="text-[10px] text-fg-dim ml-2 group-hover:text-fg-secondary transition-colors">
                  Space
                </span>
              </button>
            </div>
          ) : (
            <RatingButtons onRate={rate} />
          )
        ) : !revealed ? (
          <AnswerInput
            onSubmit={(answer) => {
              setTypedAnswer(answer);
              submitAnswer(answer);
            }}
            grading={grading}
            disabled={!current}
          />
        ) : (
          <RatingButtons onRate={rate} suggestedRating={gradeResult?.suggestedRating} />
        )}

        {/* Footer actions */}
        <div className="flex items-center justify-center gap-4">
          <button
            type="button"
            onClick={() => setEditing(true)}
            disabled={!revealed || !current}
            className={`flex items-center gap-1 text-ui-xs transition-colors ${
              revealed && current
                ? "text-fg-secondary hover:text-fg cursor-pointer"
                : "text-fg-dim cursor-not-allowed"
            }`}
          >
            <Edit3 size={11} strokeWidth={1.5} />
            Edit
            <span className="text-[10px] text-fg-dim ml-0.5">E</span>
          </button>
          <button
            type="button"
            onClick={() => {
              if (current?.sourceNoteId) {
                navigate(`/notes?id=${current.sourceNoteId}`);
              }
            }}
            disabled={!current?.sourceNoteId}
            className={`flex items-center gap-1 text-ui-xs transition-colors ${
              current?.sourceNoteId
                ? "text-fg-secondary hover:text-fg cursor-pointer"
                : "text-fg-dim cursor-not-allowed"
            }`}
          >
            <ExternalLink size={11} strokeWidth={1.5} />
            Source
          </button>
        </div>
      </div>
    </div>
  );
}
