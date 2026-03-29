import { useRef } from "react";
import { VoiceBrainOrb } from "@features/voice";
import { useTransparentBackground } from "@shared/hooks/useTransparentBackground";
import { useWindowAutoResize } from "@shared/hooks/useWindowAutoResize";

export function VoiceOrbPage() {
  const contentRef = useRef<HTMLDivElement>(null);

  useTransparentBackground({ nativeVibrancy: true });
  useWindowAutoResize(contentRef, { width: 320, maxHeight: 400 });

  return (
    <div ref={contentRef}>
      <VoiceBrainOrb />
    </div>
  );
}
