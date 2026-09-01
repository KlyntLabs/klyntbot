import type { AppSettings } from "@/types";

/**
 * Returns { checked, onChange } for a boolean AppSettings field.
 * Eliminates the repetitive `onUpdate({ ...appSettings, key: !appSettings.key })` pattern.
 */
export function useAppSettingToggle<K extends keyof AppSettings>(
  appSettings: AppSettings,
  onUpdate: (next: AppSettings) => void,
  key: K,
) {
  const value = appSettings[key];
  const checked = typeof value === "boolean" ? value : Boolean(value);
  return {
    checked,
    onChange: (nextChecked: boolean) => {
      onUpdate({ ...appSettings, [key]: nextChecked });
    },
  } as const;
}
