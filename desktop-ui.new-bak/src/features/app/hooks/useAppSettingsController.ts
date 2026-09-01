import { runCodexUpdate } from "@services/tauri";
import { useAppSettings } from "@settings/hooks/useAppSettings";
import { useThemePreference } from "@/features/layout/hooks/useThemePreference";
import { useTransparencyPreference } from "@/features/layout/hooks/useTransparencyPreference";
import { useUiScaleShortcuts } from "@/features/layout/hooks/useUiScaleShortcuts";

export function useAppSettingsController() {
  const {
    settings: appSettings,
    setSettings: setAppSettings,
    saveSettings,
    doctor,
    isLoading: appSettingsLoading,
  } = useAppSettings();

  useThemePreference(appSettings.theme);
  const { reduceTransparency, setReduceTransparency } = useTransparencyPreference();

  const { uiScale, scaleShortcutTitle, scaleShortcutText, queueSaveSettings } = useUiScaleShortcuts(
    {
      settings: appSettings,
      setSettings: setAppSettings,
      saveSettings,
    },
  );

  return {
    appSettings,
    setAppSettings,
    saveSettings,
    queueSaveSettings,
    doctor,
    codexUpdate: (codexBin: string | null, codexArgs: string | null) =>
      runCodexUpdate(codexBin, codexArgs),
    appSettingsLoading,
    reduceTransparency,
    setReduceTransparency,
    uiScale,
    scaleShortcutTitle,
    scaleShortcutText,
  };
}
