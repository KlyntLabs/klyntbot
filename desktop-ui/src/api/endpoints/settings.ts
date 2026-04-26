import type {
  AppSettings,
  CodexDoctorResult,
  CodexUpdateResult,
  TcpDaemonStatus,
  TailscaleDaemonCommandPreview,
  TailscaleStatus,
} from "@/types";
import { invoke } from "../client";

export async function getAppSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("get_app_settings");
}

export async function isMobileRuntime(): Promise<boolean> {
  return invoke<boolean>("is_mobile_runtime");
}

export async function updateAppSettings(settings: AppSettings): Promise<AppSettings> {
  return invoke<AppSettings>("update_app_settings", { settings });
}

export async function getConfigModel(workspaceId: string): Promise<string | null> {
  const response = await invoke<{ model?: string | null }>("get_config_model", {
    workspaceId,
  });
  const model = response?.model;
  if (typeof model !== "string") {
    return null;
  }
  const trimmed = model.trim();
  return trimmed.length > 0 ? trimmed : null;
}

export type MenuAcceleratorUpdate = {
  id: string;
  accelerator: string | null;
};

export async function setMenuAccelerators(
  updates: MenuAcceleratorUpdate[],
): Promise<void> {
  return invoke("menu_set_accelerators", { updates });
}

export async function runCodexDoctor(
  codexBin: string | null,
  codexArgs: string | null,
): Promise<CodexDoctorResult> {
  return invoke<CodexDoctorResult>("codex_doctor", { codexBin, codexArgs });
}

export async function runCodexUpdate(
  codexBin: string | null,
  codexArgs: string | null,
): Promise<CodexUpdateResult> {
  return invoke<CodexUpdateResult>("codex_update", { codexBin, codexArgs });
}

export type AppBuildType = "debug" | "release";

export async function getAppBuildType(): Promise<AppBuildType> {
  return invoke<AppBuildType>("app_build_type");
}

export async function tailscaleStatus(): Promise<TailscaleStatus> {
  return invoke<TailscaleStatus>("tailscale_status");
}

export async function tailscaleDaemonCommandPreview(): Promise<TailscaleDaemonCommandPreview> {
  return invoke<TailscaleDaemonCommandPreview>("tailscale_daemon_command_preview");
}

export async function tailscaleDaemonStart(): Promise<TcpDaemonStatus> {
  return invoke<TcpDaemonStatus>("tailscale_daemon_start");
}

export async function tailscaleDaemonStop(): Promise<TcpDaemonStatus> {
  return invoke<TcpDaemonStatus>("tailscale_daemon_stop");
}

export async function tailscaleDaemonStatus(): Promise<TcpDaemonStatus> {
  return invoke<TcpDaemonStatus>("tailscale_daemon_status");
}

export async function setCodexFeatureFlag(
  featureKey: string,
  enabled: boolean,
): Promise<void> {
  return invoke("set_codex_feature_flag", { featureKey, enabled });
}

export async function getExperimentalFeatureList(
  workspaceId: string,
  cursor?: string | null,
  limit?: number | null,
) {
  return invoke<any>("experimental_feature_list", { workspaceId, cursor, limit });
}
