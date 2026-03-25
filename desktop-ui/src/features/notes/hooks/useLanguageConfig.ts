import { useQuery } from "@shared/hooks/useQuery";
import { useMemo } from "react";

interface LanguageConfigResponse {
  nativeLang: string | null;
  sourceLang: string | null;
  targetLang: string | null;
  autoDetect: boolean;
  proficiencyLevel: string | null;
}

const DEFAULT_CONFIG: LanguageConfigResponse = {
  nativeLang: null,
  sourceLang: null,
  targetLang: null,
  autoDetect: true,
  proficiencyLevel: null,
};

/** Detect likely source language from text using Unicode script detection. */
function detectLanguage(text: string): string {
  const japanesePattern = /[\u3040-\u309F\u30A0-\u30FF]/;
  const koreanPattern = /[\uAC00-\uD7AF\u1100-\u11FF]/;
  const cjkPattern = /[\u2E80-\u9FFF\uF900-\uFAFF]/;
  const vietnamesePattern = /[\u1E00-\u1EFF\u01A0-\u01B0\u1EA0-\u1EF9]/;
  const arabicPattern = /[\u0600-\u06FF\u0750-\u077F]/;
  const thaiPattern = /[\u0E00-\u0E7F]/;
  const devanagariPattern = /[\u0900-\u097F]/;
  const cyrillicPattern = /[\u0400-\u04FF]/;
  if (japanesePattern.test(text)) return "ja";
  if (koreanPattern.test(text)) return "ko";
  if (cjkPattern.test(text)) return "zh";
  if (thaiPattern.test(text)) return "th";
  if (arabicPattern.test(text)) return "ar";
  if (devanagariPattern.test(text)) return "hi";
  if (cyrillicPattern.test(text)) return "ru";
  if (vietnamesePattern.test(text)) return "vi";
  return "en";
}

/**
 * Resolves source/target language pair for translation.
 *
 * Requires `nativeLang` and `targetLang` (learning language) in config.
 * Logic (after per-note overrides):
 *  - Detect what language the source text is in
 *  - If source is the learning language → translate to nativeLang (review mode)
 *  - Otherwise → translate to learning language (study mode)
 *
 * When unconfigured, falls back to en as a lingua franca.
 */
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

  const nativeLang = settings?.nativeLang ?? "en";
  const learningLang = settings?.targetLang;

  // Resolution order: per-note override → smart detection → fallback
  const sourceLang = useMemo(() => {
    if (noteOverride?.sourceLang) return noteOverride.sourceLang;
    if (settings?.sourceLang) return settings.sourceLang;
    if (sourceText) return detectLanguage(sourceText);
    return nativeLang;
  }, [noteOverride, settings, sourceText, nativeLang]);

  const targetLang = useMemo(() => {
    if (noteOverride?.targetLang) return noteOverride.targetLang;

    // No learning language configured → translate to native
    if (!learningLang) return nativeLang === sourceLang ? "en" : nativeLang;

    // Source is already the learning language → translate to native (review mode)
    if (sourceLang === learningLang) return nativeLang;

    // Otherwise → translate to learning language (study mode)
    return learningLang;
  }, [noteOverride, sourceLang, learningLang, nativeLang]);

  return {
    sourceLang,
    targetLang,
    proficiencyLevel: settings?.proficiencyLevel ?? null,
  };
}
