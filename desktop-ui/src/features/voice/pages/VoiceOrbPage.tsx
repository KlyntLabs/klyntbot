import { VoiceBrainOrb } from "@features/voice";
import { useTransparentBackground } from "@shared/hooks/useTransparentBackground";
import { useWindowAutoResize } from "@shared/hooks/useWindowAutoResize";
import { useRef } from "react";

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
