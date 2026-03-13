import { useCallback, useEffect, useRef, useState } from "react";

interface UseTypewriterOptions {
  text: string;
  speed?: number; // ms per character, default 30
  onComplete?: () => void;
}

export function useTypewriter({ text, speed = 30, onComplete }: UseTypewriterOptions) {
  const [displayed, setDisplayed] = useState("");
  const [isAnimating, setIsAnimating] = useState(false);
  const timerRef = useRef<ReturnType<typeof setInterval>>();
  const indexRef = useRef(0);
  const completeRef = useRef(onComplete);
  completeRef.current = onComplete;

  const skip = useCallback(() => {
    if (timerRef.current) clearInterval(timerRef.current);
    setDisplayed(text);
    setIsAnimating(false);
    completeRef.current?.();
  }, [text]);

  useEffect(() => {
    if (!text) {
      setDisplayed("");
      setIsAnimating(false);
      return;
    }

    indexRef.current = 0;
    setDisplayed("");
    setIsAnimating(true);

    timerRef.current = setInterval(() => {
      indexRef.current += 1;
      const next = text.slice(0, indexRef.current);
      setDisplayed(next);

      if (indexRef.current >= text.length) {
        clearInterval(timerRef.current);
        setIsAnimating(false);
        completeRef.current?.();
      }
    }, speed);

    return () => {
      if (timerRef.current) clearInterval(timerRef.current);
    };
  }, [text, speed]);

  return { displayed, isAnimating, skip };
}
