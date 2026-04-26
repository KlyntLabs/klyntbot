import { useCallback, useMemo, useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import type { AppSettings, CodexFeature } from "@/types";
import {
  getCodexConfigPath,
  getExperimentalFeatureList,
  setCodexFeatureFlag,
} from "@services/tauri";
import { qk, useTauriMutation, useTauriQuery } from "@/lib/query";

type UseSettingsFeaturesSectionArgs = {
  appSettings: AppSettings;
  featureWorkspaceId: string | null;
  onUpdateAppSettings: (next: AppSettings) => Promise<void>;
};

const HIDDEN_DYNAMIC_FEATURE_KEYS = new Set<string>([
  "personality",
  "collab",
  "steer",
]);

export type SettingsFeaturesSectionProps = {
  appSettings: AppSettings;
  hasFeatureWorkspace: boolean;
  openConfigError: string | null;
  featureError: string | null;
  featuresLoading: boolean;
  featureUpdatingKey: string | null;
  stableFeatures: CodexFeature[];
  experimentalFeatures: CodexFeature[];
  hasDynamicFeatureRows: boolean;
  onOpenConfig: () => void;
  onToggleCodexFeature: (feature: CodexFeature) => void;
  onUpdateAppSettings: (next: AppSettings) => Promise<void>;
};

function mapFeatureToAppSettings(
  appSettings: AppSettings,
  featureKey: string,
  enabled: boolean,
): AppSettings | null {
  if (featureKey === "apps") {
    return { ...appSettings, experimentalAppsEnabled: enabled };
  }
  if (featureKey === "collaboration_modes") {
    return { ...appSettings, collaborationModesEnabled: enabled };
  }
  if (featureKey === "steer") {
    return { ...appSettings, steerEnabled: enabled };
  }
  if (featureKey === "unified_exec") {
    return { ...appSettings, unifiedExecEnabled: enabled };
  }
  return null;
}

export const useSettingsFeaturesSection = ({
  appSettings,
  featureWorkspaceId,
  onUpdateAppSettings,
}: UseSettingsFeaturesSectionArgs): SettingsFeaturesSectionProps => {
  const [openConfigError, setOpenConfigError] = useState<string | null>(null);
  const [featureError, setFeatureError] = useState<string | null>(null);
  const [featureUpdatingKey, setFeatureUpdatingKey] = useState<string | null>(
    null,
  );

  const featuresQuery = useTauriQuery<CodexFeature[]>({
    queryKey: qk.settings.features(featureWorkspaceId),
    queryFn: async () => {
      if (!featureWorkspaceId) return [];
      const collected: CodexFeature[] = [];
      let cursor: string | null = null;
      for (let page = 0; page < 20; page += 1) {
        const result = await getExperimentalFeatureList(
          featureWorkspaceId,
          cursor,
          100,
        );
        collected.push(...result.features);
        if (!result.nextCursor) break;
        cursor = result.nextCursor;
      }
      return collected;
    },
    fallback: [],
    enabled: featureWorkspaceId !== null,
  });

  const configPathQuery = useTauriQuery<string | null>({
    queryKey: qk.settings.codexConfigPath(),
    queryFn: () => getCodexConfigPath(),
    fallback: null,
  });

  const toggleFeature = useTauriMutation<
    void,
    { name: string; enabled: boolean }
  >({
    mutationFn: (input) => setCodexFeatureFlag(input.name, input.enabled),
    invalidates: [qk.settings.features(featureWorkspaceId)],
  });

  const handleOpenConfig = useCallback(async () => {
    setOpenConfigError(null);
    try {
      const configPath = configPathQuery.data;
      if (!configPath) return;
      await revealItemInDir(configPath);
    } catch (error) {
      setOpenConfigError(
        error instanceof Error ? error.message : "Unable to open config.",
      );
    }
  }, [configPathQuery.data]);

  const stableFeatures = useMemo(
    () =>
      featuresQuery.data.filter(
        (feature) =>
          feature.stage === "stable" &&
          !HIDDEN_DYNAMIC_FEATURE_KEYS.has(feature.name),
      ),
    [featuresQuery.data],
  );
  const experimentalFeatures = useMemo(
    () =>
      featuresQuery.data.filter(
        (feature) =>
          (feature.stage === "beta" || feature.stage === "under_development") &&
          !HIDDEN_DYNAMIC_FEATURE_KEYS.has(feature.name),
      ),
    [featuresQuery.data],
  );
  const hasDynamicFeatureRows =
    stableFeatures.length > 0 || experimentalFeatures.length > 0;

  const onToggleCodexFeature = useCallback(
    (feature: CodexFeature) => {
      void (async () => {
        const nextEnabled = !feature.enabled;
        setFeatureUpdatingKey(feature.name);
        setFeatureError(null);
        try {
          const nextSettings = mapFeatureToAppSettings(
            appSettings,
            feature.name,
            nextEnabled,
          );
          if (nextSettings) {
            await onUpdateAppSettings(nextSettings);
          } else {
            await toggleFeature.mutate({
              name: feature.name,
              enabled: nextEnabled,
            });
          }
        } catch (error) {
          setFeatureError(
            error instanceof Error
              ? error.message
              : `Unable to update feature "${feature.name}".`,
          );
        } finally {
          setFeatureUpdatingKey((current) =>
            current === feature.name ? null : current,
          );
        }
      })();
    },
    [appSettings, onUpdateAppSettings, toggleFeature],
  );

  return {
    appSettings,
    hasFeatureWorkspace: featureWorkspaceId != null,
    openConfigError,
    featureError,
    featuresLoading: featuresQuery.isLoading,
    featureUpdatingKey,
    stableFeatures,
    experimentalFeatures,
    hasDynamicFeatureRows,
    onOpenConfig: () => {
      void handleOpenConfig();
    },
    onToggleCodexFeature,
    onUpdateAppSettings,
  };
};
