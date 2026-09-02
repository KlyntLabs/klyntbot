import type { ReactNode } from "react";
import { useTypewriter } from "../hooks/useTypewriter";

interface TypewriterTextProps {
  text: string; // Full prompt with {input} placeholder
  input?: ReactNode; // The inline input component
  onComplete?: () => void;
  showInput?: boolean; // Show input after typewriter finishes
}

export function TypewriterText({
  text,
  input,
  onComplete,
  showInput = false,
}: TypewriterTextProps) {
  const parts = text.split("{input}");
  const before = parts[0] ?? "";
  const after = parts[1] ?? "";
  const hasInput = text.includes("{input}");

  // Animate only the "before" part — stop at the blank
  const { displayed, isAnimating, skip } = useTypewriter({
    text: before,
    onComplete,
  });

  const handleClick = () => {
    if (isAnimating) skip();
  };

  // During animation: typewriter the text before the blank
  if (isAnimating) {
    return (
      <button type="button" onClick={handleClick} className="cursor-pointer text-left">
        {displayed}
      </button>
    );
  }

  // After animation: show before + (blank or input) + after
  if (!showInput && hasInput) {
    // Show a visual blank placeholder (underlined space)
    return (
      <button type="button" onClick={handleClick} className="cursor-pointer text-left">
        {before}
        <span className="inline-block border-b-2 border-brand/40 min-w-[80px]">&nbsp;</span>
        {after}
      </button>
    );
  }

  // Show: before + live input + after
  return (
    <span>
      {before}
      {hasInput ? input : null}
      {after}
    </span>
  );
}
