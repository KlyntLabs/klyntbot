import { useQuery } from "@shared/hooks/useQuery";
import { useMemo } from "react";

interface LanguageConfigResponse {
  sourceLang: string | null;
  targetLang: string | null;
  autoDetect: boolean;
  proficiencyLevel: string | null;
}

const DEFAULT_CONFIG: LanguageConfigResponse = {
  sourceLang: null,
  targetLang: null,
  autoDetect: true,
  proficiencyLevel: null,
};

/** Detect likely source language from text using Unicode script detection. */
function detectLanguage(text: string): string {
  const japanesePattern = /[\u3040-\u309F\u30A0-\u30FF]/;
  const cjkPattern = /[\u2E80-\u9FFF\uF900-\uFAFF]/;
  if (japanesePattern.test(text)) return "ja";
  if (cjkPattern.test(text)) return "zh";
  return "en";
}

export function useLanguageConfig(
  perspectiveConfig: string | null | undefined,
  sourceText?: string,
) {
  const { data: settings } = useQuery<LanguageConfigResponse>(
    "config_get_section",
    { section: "language" },
    DEFAULT_CONFIG,
  );

  // Check for per-note override in perspectiveConfig
  const noteOverride = useMemo(() => {
    if (!perspectiveConfig) return null;
    try {
      const config = JSON.parse(perspectiveConfig);
      return config.languagePair ?? null;
    } catch {
      return null;
    }
  }, [perspectiveConfig]);

  // Resolution order: per-note → global → auto-detect
  const sourceLang = useMemo(() => {
    if (noteOverride?.sourceLang) return noteOverride.sourceLang;
    if (settings?.sourceLang) return settings.sourceLang;
    if (sourceText) return detectLanguage(sourceText);
    return "zh";
  }, [noteOverride, settings, sourceText]);

  const targetLang = useMemo(() => {
    if (noteOverride?.targetLang) return noteOverride.targetLang;
    if (settings?.targetLang) return settings.targetLang;
    return "en";
  }, [noteOverride, settings]);

  return {
    sourceLang,
    targetLang,
    proficiencyLevel: settings?.proficiencyLevel ?? null,
  };
}
