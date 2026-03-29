import { VoiceBrainOrb } from "@features/voice";
import { useTransparentBackground } from "@shared/hooks/useTransparentBackground";

export function VoiceOrbPage() {
  useTransparentBackground({ nativeVibrancy: true });

  return (
    <div className="h-screen w-screen">
      <VoiceBrainOrb />
    </div>
  );
}
