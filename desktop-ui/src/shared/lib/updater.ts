import { isTauri } from "@shared/lib/utils";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

let cachedUpdate: Update | null = null;

export async function checkUpdateAvailable(): Promise<{ available: boolean; version?: string }> {
  if (!isTauri) return { available: false };
  try {
    const update = await check();
    if (update) {
      cachedUpdate = update;
      return { available: true, version: update.version };
    }
    return { available: false };
  } catch (e) {
    console.warn("Update check failed:", e);
    return { available: false };
  }
}

export async function installUpdate(): Promise<void> {
  if (!cachedUpdate) {
    const result = await checkUpdateAvailable();
    if (!result.available || !cachedUpdate) return;
  }
  const confirmed = window.confirm(`Version ${cachedUpdate.version} is available. Update now?`);
  if (confirmed) {
    await cachedUpdate.downloadAndInstall();
    await relaunch();
  }
}

export async function checkForUpdates(): Promise<void> {
  const result = await checkUpdateAvailable();
  if (result.available) {
    await installUpdate();
  }
}
