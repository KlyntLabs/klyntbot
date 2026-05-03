import type { AppMention, ReviewTarget, TrayRecentThreadEntry, TraySessionUsage } from "@/types";
import { invoke, isMissingTauriInvokeError } from "../client";
import { convertImagesToDataUrls } from "./files";
import { getAppSettings, isMobileRuntime } from "./settings";

async function normalizeImagesForRpc(images?: string[]): Promise<string[] | null> {
  if (images == null) {
    return null;
  }
  if (images.length === 0) {
    return [];
  }
  const hasPathImages = images.some(
    (image) =>
      !image.startsWith("data:") && !image.startsWith("http://") && !image.startsWith("https://"),
  );
  if (!hasPathImages) {
    return images;
  }
  let settings: Awaited<ReturnType<typeof getAppSettings>>;
  let mobileRuntime: boolean;
  try {
    [settings, mobileRuntime] = await Promise.all([getAppSettings(), isMobileRuntime()]);
  } catch (error) {
    if (isMissingTauriInvokeError(error)) {
      return images;
    }
    throw error;
  }
  if (settings.backendMode !== "remote" && !mobileRuntime) {
    return images;
  }
  return convertImagesToDataUrls(images);
}

export async function startThread(workspaceId: string) {
  return invoke<Record<string, unknown>>("coding_thread_start", { workspaceId, model: null, approvalPolicy: null, ephemeral: false });
}

export async function forkThread(workspaceId: string, threadId: string) {
  void workspaceId;
  return invoke<Record<string, unknown>>("coding_thread_fork", { threadId, fromMessageId: null });
}

export async function compactThread(workspaceId: string, threadId: string) {
  void workspaceId;
  return invoke<Record<string, unknown>>("coding_thread_compact", { threadId });
}

export async function sendUserMessage(
  workspaceId: string,
  threadId: string,
  text: string,
  options?: {
    model?: string | null;
    effort?: string | null;
    serviceTier?: "fast" | "flex" | null | undefined;
    accessMode?: "read-only" | "current" | "full-access";
    images?: string[];
    collaborationMode?: Record<string, unknown> | null;
    appMentions?: AppMention[];
  },
) {
  void workspaceId;
  void options?.effort;
  void options?.serviceTier;
  void options?.accessMode;
  void options?.collaborationMode;
  void options?.appMentions;
  const images = await normalizeImagesForRpc(options?.images);
  void images;
  return invoke("coding_message_send", {
    threadId,
    text,
    model: options?.model ?? null,
  });
}

export async function interruptTurn(workspaceId: string, threadId: string, turnId: string) {
  void workspaceId;
  return invoke("coding_turn_interrupt", { threadId, turnId });
}

export async function steerTurn(
  workspaceId: string,
  threadId: string,
  turnId: string,
  text: string,
  images?: string[],
  appMentions?: AppMention[],
) {
  void workspaceId;
  void images;
  void appMentions;
  return invoke("coding_turn_steer", { threadId, turnId, text });
}

export async function startReview(
  workspaceId: string,
  threadId: string,
  target: ReviewTarget,
  delivery?: "inline" | "detached",
) {
  const payload: Record<string, unknown> = { workspaceId, threadId, target };
  if (delivery) {
    payload.delivery = delivery;
  }
  return invoke("start_review", payload);
}

export async function respondToServerRequest(
  workspaceId: string,
  requestId: number | string,
  decision: "accept" | "decline",
) {
  void workspaceId;
  const kind = decision === "accept" ? "accept" : "decline";
  return invoke("approval_respond", {
    approvalId: String(requestId),
    decision: { kind },
  });
}

export async function respondToUserInputRequest(
  workspaceId: string,
  requestId: number | string,
  answers: Record<string, { answers: string[] }>,
) {
  void workspaceId;
  void answers;
  return invoke("approval_respond", {
    approvalId: String(requestId),
    decision: { kind: "accept" },
  });
}

export async function rememberApprovalRule(workspaceId: string, command: string[]) {
  return invoke("remember_approval_rule", { workspaceId, command });
}

export async function listThreads(
  workspaceId: string,
  cursor?: string | null,
  limit?: number | null,
  sortKey?: "created_at" | "updated_at" | null,
) {
  return invoke<Record<string, unknown>>("coding_thread_list", { workspaceId, cursor, limit, sortKey });
}

export async function listMcpServerStatus(
  workspaceId: string,
  cursor?: string | null,
  limit?: number | null,
) {
  return invoke<Record<string, unknown>>("list_mcp_server_status", { workspaceId, cursor, limit });
}

export async function resumeThread(workspaceId: string, threadId: string) {
  void workspaceId;
  return invoke<Record<string, unknown>>("coding_thread_resume", { threadId, includeItems: true });
}

export async function readThread(workspaceId: string, threadId: string) {
  void workspaceId;
  return invoke<Record<string, unknown>>("coding_thread_read", { threadId, cursor: null, limit: null });
}

export async function threadLiveSubscribe(workspaceId: string, threadId: string) {
  void workspaceId;
  return invoke<Record<string, unknown>>("coding_thread_subscribe", { threadId });
}

export async function threadLiveUnsubscribe(workspaceId: string, threadId: string) {
  void workspaceId;
  return invoke<Record<string, unknown>>("coding_thread_unsubscribe", { subscriptionId: threadId });
}

export async function archiveThread(workspaceId: string, threadId: string) {
  void workspaceId;
  return invoke<Record<string, unknown>>("coding_thread_archive", { threadId });
}

export async function setThreadName(workspaceId: string, threadId: string, name: string) {
  void workspaceId;
  return invoke<Record<string, unknown>>("coding_thread_set_name", { threadId, name });
}

export async function setTrayRecentThreads(entries: TrayRecentThreadEntry[]) {
  return invoke<void>("set_tray_recent_threads", { entries });
}

export async function setTraySessionUsage(usage: TraySessionUsage | null) {
  return invoke<void>("set_tray_session_usage", { usage });
}

export async function generateRunMetadata(workspaceId: string, prompt: string) {
  return invoke<{ title: string; worktreeName: string }>("generate_run_metadata", {
    workspaceId,
    prompt,
  });
}

export async function getAccountRateLimits(workspaceId: string) {
  return invoke<Record<string, unknown>>("account_rate_limits", { workspaceId });
}

export async function getAccountInfo(workspaceId: string) {
  return invoke<Record<string, unknown>>("account_read", { workspaceId });
}

export async function runCodexLogin(workspaceId: string) {
  return invoke<{ loginId: string; authUrl: string; raw?: unknown }>("codex_login", {
    workspaceId,
  });
}

export async function cancelCodexLogin(workspaceId: string) {
  return invoke<{ canceled: boolean; status?: string; raw?: unknown }>("codex_login_cancel", {
    workspaceId,
  });
}
