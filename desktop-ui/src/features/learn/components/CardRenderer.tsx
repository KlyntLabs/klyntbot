import { ImageOff } from "lucide-react";
import { useMemo, useState } from "react";
import type { Flashcard } from "../../notes/hooks/useFlashcards";

interface CardRendererProps {
  card: Flashcard;
  revealed: boolean;
}

function BasicCard({ card, revealed }: { card: Flashcard; revealed: boolean }) {
  return (
    <div className="text-center space-y-6">
      <div className="text-lg text-fg whitespace-pre-wrap">{card.front}</div>
      {revealed && (
        <div className="animate-[fade-in-up_0.25s_ease-out]">
          <div className="glass-divider mb-6" />
          <div className="text-lg text-fg whitespace-pre-wrap">{card.back}</div>
        </div>
      )}
    </div>
  );
}

function ClozeCard({ card, revealed }: { card: Flashcard; revealed: boolean }) {
  const maskedText = useMemo(() => {
    return card.front.replace(/\{\{c\d+::([^}]+?)(?:::[^}]+?)?\}\}/g, "[...]");
  }, [card.front]);

  const fullText = useMemo(() => {
    return card.front.replace(/\{\{c\d+::([^}]+?)(?:::[^}]+?)?\}\}/g, "$1");
  }, [card.front]);

  return (
    <div className="text-center space-y-6">
      <div className="text-lg text-fg whitespace-pre-wrap">
        {revealed ? fullText : maskedText}
      </div>
      {revealed && card.back && (
        <div className="animate-[fade-in-up_0.25s_ease-out]">
          <div className="glass-divider mb-6" />
          <p className="text-sm text-fg-secondary">{card.back}</p>
        </div>
      )}
    </div>
  );
}

function VocabularyCard({ card, revealed }: { card: Flashcard; revealed: boolean }) {
  const vocab = card.vocabData;
  if (!vocab) return <BasicCard card={card} revealed={revealed} />;

  return (
    <div className="text-center space-y-6">
      <div>
        <p className="text-2xl font-semibold text-fg">{vocab.word ?? card.front}</p>
        {vocab.reading && <p className="text-sm text-fg-secondary mt-1">{vocab.reading}</p>}
        {vocab.partOfSpeech && (
          <span className="inline-block mt-2 text-ui-xs text-fg-secondary glass-badge px-2 py-0.5">
            {vocab.partOfSpeech}
          </span>
        )}
      </div>
      {revealed && (
        <div className="animate-[fade-in-up_0.25s_ease-out] space-y-4">
          <div className="glass-divider" />
          <p className="text-lg text-fg">{vocab.meaning ?? card.back}</p>
          {vocab.exampleSentence && (
            <p className="text-sm text-fg-secondary italic">{vocab.exampleSentence}</p>
          )}
        </div>
      )}
    </div>
  );
}

function TypedCard({ card, revealed }: { card: Flashcard; revealed: boolean }) {
  const [typed, setTyped] = useState("");
  const isCorrect = revealed && typed.trim().toLowerCase() === card.back.trim().toLowerCase();

  return (
    <div className="text-center space-y-6">
      <div className="text-lg text-fg whitespace-pre-wrap">{card.front}</div>
      <input
        type="text"
        value={typed}
        onChange={(e) => setTyped(e.target.value)}
        disabled={revealed}
        placeholder="Type your answer..."
        className="glass-input w-full max-w-sm mx-auto px-3 py-2 text-sm text-fg text-center"
      />
      {revealed && (
        <div className="animate-[fade-in-up_0.25s_ease-out] space-y-2">
          <div className="glass-divider" />
          <p className={`text-sm font-medium ${isCorrect ? "text-emerald-400" : "text-red-400"}`}>
            {isCorrect ? "Correct!" : "Incorrect"}
          </p>
          <p className="text-lg text-fg">{card.back}</p>
        </div>
      )}
    </div>
  );
}

function ImageOcclusionCard() {
  return (
    <div className="text-center py-8">
      <ImageOff size={32} className="mx-auto text-fg-secondary mb-3" strokeWidth={1.5} />
      <p className="text-sm text-fg-secondary">Image cards coming soon</p>
    </div>
  );
}

export function CardRenderer({ card, revealed }: CardRendererProps) {
  switch (card.cardType) {
    case "cloze":
      return <ClozeCard card={card} revealed={revealed} />;
    case "vocabulary":
      return <VocabularyCard card={card} revealed={revealed} />;
    case "typed":
      return <TypedCard card={card} revealed={revealed} />;
    case "image_occlusion":
      return <ImageOcclusionCard />;
    default:
      return <BasicCard card={card} revealed={revealed} />;
  }
}
