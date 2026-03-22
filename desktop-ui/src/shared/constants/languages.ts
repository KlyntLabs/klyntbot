export interface Language {
  code: string;
  label: string;
  native: string;
  flag: string;
}

export const LANGUAGES: Language[] = [
  { code: "zh", label: "Chinese", native: "中文", flag: "🇨🇳" },
  { code: "ja", label: "Japanese", native: "日本語", flag: "🇯🇵" },
  { code: "ko", label: "Korean", native: "한국어", flag: "🇰🇷" },
  { code: "vi", label: "Vietnamese", native: "Tiếng Việt", flag: "🇻🇳" },
  { code: "en", label: "English", native: "English", flag: "🇬🇧" },
  { code: "es", label: "Spanish", native: "Español", flag: "🇪🇸" },
  { code: "fr", label: "French", native: "Français", flag: "🇫🇷" },
  { code: "de", label: "German", native: "Deutsch", flag: "🇩🇪" },
  { code: "ru", label: "Russian", native: "Русский", flag: "🇷🇺" },
  { code: "ar", label: "Arabic", native: "العربية", flag: "🇸🇦" },
  { code: "th", label: "Thai", native: "ไทย", flag: "🇹🇭" },
  { code: "hi", label: "Hindi", native: "हिन्दी", flag: "🇮🇳" },
  { code: "pt", label: "Portuguese", native: "Português", flag: "🇵🇹" },
];

export function findLanguage(code: string): Language | undefined {
  return LANGUAGES.find((l) => l.code === code);
}
