import { useEffect, useRef, useState } from "react";

interface DocResult {
  index: number;
  finalTranslation: string;
  grade: string;
}

interface PracticeDocPanelProps {
  results: DocResult[];
  currentIndex: number;
  totalSegments: number;
  onGradeClick?: (index: number) => void;
}

function gradeColorClass(grade: string): string {
  const upper = grade.toUpperCase();
  if (upper.startsWith("A")) return "text-green-400";
  if (upper.startsWith("B")) return "text-yellow-400";
  if (upper.startsWith("C")) return "text-orange-400";
  return "text-red-400";
}

export function PracticeDocPanel({
  results,
  currentIndex: _currentIndex,
  totalSegments,
  onGradeClick,
}: PracticeDocPanelProps) {
  const bottomRef = useRef<HTMLDivElement>(null);
  const [flashIndex, setFlashIndex] = useState<number | null>(null);

  // Auto-scroll to bottom when new translations are added
  useEffect(() => {
    if (results.length > 0) {
      bottomRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [results.length]);

  // Green flash animation on new translation
  useEffect(() => {
    if (results.length === 0) return;
    const latest = results[results.length - 1];
    setFlashIndex(latest.index);
    const timer = setTimeout(() => setFlashIndex(null), 600);
    return () => clearTimeout(timer);
  }, [results]);

  const isComplete = results.length >= totalSegments;

  return (
    <div className="p-4 space-y-3 overflow-auto h-full">
      {results.map((result) => (
        <div
          key={result.index}
          className="transition-opacity duration-500"
          style={{ opacity: flashIndex === result.index ? 0 : 1 }}
          ref={(el) => {
            // Trigger reflow to animate from 0 to 1
            if (el && flashIndex === result.index) {
              requestAnimationFrame(() => {
                el.style.opacity = "1";
              });
            }
          }}
        >
          <p className="text-sm leading-relaxed text-brand">
            {result.finalTranslation}
            <button
              type="button"
              onClick={() => onGradeClick?.(result.index)}
              className={`ml-2 text-ui-sm font-medium cursor-pointer hover:underline ${gradeColorClass(result.grade)}`}
            >
              {result.grade}
            </button>
          </p>
        </div>
      ))}

      {/* Current unit placeholder */}
      {!isComplete && (
        <p className="text-fg-secondary italic text-sm">Waiting for your translation...</p>
      )}

      <div ref={bottomRef} />
    </div>
  );
}
