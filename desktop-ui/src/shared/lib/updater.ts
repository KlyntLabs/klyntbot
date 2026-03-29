import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import { isTauri } from "@shared/lib/utils";

export async function checkForUpdates(): Promise<void> {
  if (!isTauri) return;
  try {
    const update = await check();
    if (update) {
      const confirmed = window.confirm(`Version ${update.version} is available. Update now?`);
      if (confirmed) {
        await update.downloadAndInstall();
        await relaunch();
      }
    }
  } catch (e) {
    console.warn("Update check failed:", e);
  }
}
