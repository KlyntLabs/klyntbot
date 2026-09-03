import { useState } from "react";

interface VoiceConfig {
  enabled: boolean;
  input: {
    hotkey: string;
    silenceThresholdSecs: number;
    privacyMode: "standard" | "strict" | "off";
    modelSize: "small" | "medium";
  };
  output: {
    enabled: boolean;
    speakingRate: number;
    speakDuringFocus: boolean;
  };
  learning: {
    targetLanguage: string | null;
    showPronunciationScores: boolean;
    autoCreateFlashcards: boolean;
  };
}

export function VoiceSettings() {
  const [config] = useState<VoiceConfig>({
    enabled: true,
    input: {
      hotkey: "alt+shift+v",
      silenceThresholdSecs: 1.5,
      privacyMode: "standard",
      modelSize: "small",
    },
    output: {
      enabled: true,
      speakingRate: 1.0,
      speakDuringFocus: false,
    },
    learning: {
      targetLanguage: null,
      showPronunciationScores: true,
      autoCreateFlashcards: true,
    },
  });

  return (
    <div className="space-y-6">
      <h3 className="text-lg font-medium text-fg">Voice</h3>

      <section className="space-y-3">
        <h4 className="text-sm font-medium text-fg-secondary">Voice Input</h4>
        <div className="flex items-center justify-between">
          <span className="text-sm">Enable voice capture</span>
          <span className="text-ui-sm text-fg-secondary">{config.enabled ? "On" : "Off"}</span>
        </div>
        <div className="flex items-center justify-between">
          <span className="text-sm">Global hotkey</span>
          <span className="text-ui-sm text-fg-secondary font-mono">{config.input.hotkey}</span>
        </div>
        <div className="flex items-center justify-between">
          <span className="text-sm">Silence threshold</span>
          <span className="text-ui-sm text-fg-secondary">{config.input.silenceThresholdSecs}s</span>
        </div>
        <div className="flex items-center justify-between">
          <span className="text-sm">Privacy mode</span>
          <span className="text-ui-sm text-fg-secondary capitalize">
            {config.input.privacyMode}
          </span>
        </div>
        <div className="flex items-center justify-between">
          <span className="text-sm">Transcription engine</span>
          <span className="text-ui-sm text-fg-secondary">
            Local (whisper-{config.input.modelSize})
          </span>
        </div>
      </section>

      <section className="space-y-3">
        <h4 className="text-sm font-medium text-fg-secondary">Voice Output</h4>
        <div className="flex items-center justify-between">
          <span className="text-sm">Enable spoken responses</span>
          <span className="text-ui-sm text-fg-secondary">
            {config.output.enabled ? "On" : "Off"}
          </span>
        </div>
        <div className="flex items-center justify-between">
          <span className="text-sm">Speaking rate</span>
          <span className="text-ui-sm text-fg-secondary">{config.output.speakingRate}x</span>
        </div>
        <div className="flex items-center justify-between">
          <span className="text-sm">Speak during focus sessions</span>
          <span className="text-ui-sm text-fg-secondary">
            {config.output.speakDuringFocus ? "On" : "Off"}
          </span>
        </div>
      </section>

      <section className="space-y-3">
        <h4 className="text-sm font-medium text-fg-secondary">Language Learning</h4>
        <div className="flex items-center justify-between">
          <span className="text-sm">Target language</span>
          <span className="text-ui-sm text-fg-secondary">
            {config.learning.targetLanguage || "Not set"}
          </span>
        </div>
        <div className="flex items-center justify-between">
          <span className="text-sm">Show pronunciation scores</span>
          <span className="text-ui-sm text-fg-secondary">
            {config.learning.showPronunciationScores ? "On" : "Off"}
          </span>
        </div>
        <div className="flex items-center justify-between">
          <span className="text-sm">Auto-create spoken flashcards</span>
          <span className="text-ui-sm text-fg-secondary">
            {config.learning.autoCreateFlashcards ? "On" : "Off"}
          </span>
        </div>
      </section>
    </div>
  );
}
