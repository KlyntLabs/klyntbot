import { type AppBuildType, getAppBuildType, isMobileRuntime } from "@services/tauri";
import { useEffect, useState } from "react";
import {
  SettingsField,
  SettingsFieldLabel,
  SettingsHelpText,
  SettingsSection,
  SettingsToggleRow,
  SettingsToggleSwitch,
} from "@/features/design-system/components/settings/SettingsPrimitives";
import { useUpdater } from "@/features/update/hooks/useUpdater";
import type { AppSettings } from "@/types";

type SettingsAboutSectionProps = {
  appSettings: AppSettings;
  onToggleAutomaticAppUpdateChecks?: () => void;
};

function formatBytes(value: number) {
  if (!Number.isFinite(value) || value <= 0) {
    return "0 B";
  }
  const units = ["B", "KB", "MB", "GB"];
  let size = value;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }
  return `${size.toFixed(size >= 10 ? 0 : 1)} ${units[unitIndex]}`;
}

export function SettingsAboutSection({
  appSettings,
  onToggleAutomaticAppUpdateChecks,
}: SettingsAboutSectionProps) {
  const [appBuildType, setAppBuildType] = useState<AppBuildType | "unknown">("unknown");
  const [updaterEnabled, setUpdaterEnabled] = useState(false);
  const {
    state: updaterState,
    checkForUpdates,
    startUpdate,
  } = useUpdater({
    enabled: updaterEnabled,
    autoCheckOnMount: false,
  });

  useEffect(() => {
    let active = true;
    const loadBuildType = async () => {
      try {
        const value = await getAppBuildType();
        if (active) {
          setAppBuildType(value);
        }
      } catch {
        if (active) {
          setAppBuildType("unknown");
        }
      }
    };
    void loadBuildType();
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    let active = true;
    const detectRuntime = async () => {
      try {
        const mobileRuntime = await isMobileRuntime();
        if (active) {
          setUpdaterEnabled(!mobileRuntime);
        }
      } catch {
        if (active) {
          // In non-Tauri previews we still want local desktop-like behavior.
          setUpdaterEnabled(true);
        }
      }
    };
    void detectRuntime();
    return () => {
      active = false;
    };
  }, []);

  const buildDateValue = __APP_BUILD_DATE__.trim();
  const parsedBuildDate = Date.parse(buildDateValue);
  const buildDateLabel = Number.isNaN(parsedBuildDate)
    ? buildDateValue || "unknown"
    : new Date(parsedBuildDate).toLocaleString();

  return (
    <SettingsSection title="About" subtitle="App version, build metadata, and update controls.">
      <SettingsField>
        <SettingsHelpText>
          Version: <code>{__APP_VERSION__}</code>
        </SettingsHelpText>
        <SettingsHelpText>
          Build type: <code>{appBuildType}</code>
        </SettingsHelpText>
        <SettingsHelpText>
          Branch: <code>{__APP_GIT_BRANCH__ || "unknown"}</code>
        </SettingsHelpText>
        <SettingsHelpText>
          Commit: <code>{__APP_COMMIT_HASH__ || "unknown"}</code>
        </SettingsHelpText>
        <SettingsHelpText>
          Build date: <code>{buildDateLabel}</code>
        </SettingsHelpText>
      </SettingsField>
      <SettingsField>
        <SettingsFieldLabel>App Updates</SettingsFieldLabel>
        <SettingsToggleRow
          title="Automatically check for app updates"
          subtitle="When enabled, Klynt checks for new app versions on launch."
        >
          <SettingsToggleSwitch
            pressed={appSettings.automaticAppUpdateChecksEnabled}
            onClick={() => {
              onToggleAutomaticAppUpdateChecks?.();
            }}
          />
        </SettingsToggleRow>
        <SettingsHelpText>
          Currently running version <code>{__APP_VERSION__}</code>
        </SettingsHelpText>
        {!updaterEnabled && (
          <SettingsHelpText>Updates are unavailable in this runtime.</SettingsHelpText>
        )}

        {updaterState.stage === "error" && (
          <SettingsHelpText error>Update failed: {updaterState.error}</SettingsHelpText>
        )}

        {updaterState.stage === "downloading" ||
        updaterState.stage === "installing" ||
        updaterState.stage === "restarting" ? (
          <SettingsHelpText>
            {updaterState.stage === "downloading" ? (
              <>
                Downloading update...{" "}
                {updaterState.progress?.totalBytes
                  ? `${Math.round((updaterState.progress.downloadedBytes / updaterState.progress.totalBytes) * 100)}%`
                  : formatBytes(updaterState.progress?.downloadedBytes ?? 0)}
              </>
            ) : updaterState.stage === "installing" ? (
              "Installing update..."
            ) : (
              "Restarting..."
            )}
          </SettingsHelpText>
        ) : updaterState.stage === "available" ? (
          <SettingsHelpText>
            Version <code>{updaterState.version}</code> is available.
          </SettingsHelpText>
        ) : updaterState.stage === "latest" ? (
          <SettingsHelpText>You are on the latest version.</SettingsHelpText>
        ) : null}

        <div className="flex items-center gap-2.5 mt-2">
          {updaterState.stage === "available" ? (
            <button
              type="button"
              className="primary"
              disabled={!updaterEnabled}
              onClick={() => void startUpdate()}
            >
              Download & Install
            </button>
          ) : (
            <button
              type="button"
              className="ghost"
              disabled={
                !updaterEnabled ||
                updaterState.stage === "checking" ||
                updaterState.stage === "downloading" ||
                updaterState.stage === "installing" ||
                updaterState.stage === "restarting"
              }
              onClick={() => void checkForUpdates({ announceNoUpdate: true })}
            >
              {updaterState.stage === "checking" ? "Checking..." : "Check for updates"}
            </button>
          )}
        </div>
      </SettingsField>
    </SettingsSection>
  );
}
