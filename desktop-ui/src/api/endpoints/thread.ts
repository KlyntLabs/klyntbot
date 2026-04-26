import type { AppMention, ReviewTarget, TrayRecentThreadEntry, TraySessionUsage } from "@/types";
import { invoke } from "../client";
import { convertImagesToDataUrls } from "./files";
import { getAppSettings, isMobileRuntime } from "./settings";
import { isMissingTauriInvokeError } from "../client";

async function normalizeImagesForRpc(images?: string[]): Promise<string[] | null> {
  if (images == null) {
    return null;
  }
  if (images.length === 0) {
    return [];
  }
  const hasPathImages = images.some(
    (image) =>
      !image.startsWith("data:") &&
      !image.startsWith("http://") &&
      !image.startsWith("https://"),
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
  return invoke<any>("start_thread", { workspaceId });
}

export async function forkThread(workspaceId: string, threadId: string) {
  return invoke<any>("fork_thread", { workspaceId, threadId });
}

export async function compactThread(workspaceId: string, threadId: string) {
  return invoke<any>("compact_thread", { workspaceId, threadId });
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
  const images = await normalizeImagesForRpc(options?.images);
  const payload: Record<string, unknown> = {
    workspaceId,
    threadId,
    text,
    model: options?.model ?? null,
    effort: options?.effort ?? null,
    accessMode: options?.accessMode ?? null,
    images,
  };
  if (options?.serviceTier !== undefined) {
    payload.serviceTier = options.serviceTier;
  }
  if (options?.collaborationMode) {
    payload.collaborationMode = options.collaborationMode;
  }
  if (options?.appMentions && options.appMentions.length > 0) {
    payload.appMentions = options.appMentions;
  }
  return invoke("send_user_message", payload);
}

export async function interruptTurn(
  workspaceId: string,
  threadId: string,
  turnId: string,
) {
  return invoke("turn_interrupt", { workspaceId, threadId, turnId });
}

export async function steerTurn(
  workspaceId: string,
  threadId: string,
  turnId: string,
  text: string,
  images?: string[],
  appMentions?: AppMention[],
) {
  const normalizedImages = await normalizeImagesForRpc(images);
  const payload: Record<string, unknown> = {
    workspaceId,
    threadId,
    turnId,
    text,
    images: normalizedImages,
  };
  if (appMentions && appMentions.length > 0) {
    payload.appMentions = appMentions;
  }
  return invoke("turn_steer", payload);
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
  return invoke("respond_to_server_request", {
    workspaceId,
    requestId,
    result: { decision },
  });
}

export async function respondToUserInputRequest(
  workspaceId: string,
  requestId: number | string,
  answers: Record<string, { answers: string[] }>,
) {
  return invoke("respond_to_server_request", {
    workspaceId,
    requestId,
    result: { answers },
  });
}

export async function rememberApprovalRule(
  workspaceId: string,
  command: string[],
) {
  return invoke("remember_approval_rule", { workspaceId, command });
}

export async function listThreads(
  workspaceId: string,
  cursor?: string | null,
  limit?: number | null,
  sortKey?: "created_at" | "updated_at" | null,
) {
  return invoke<any>("list_threads", { workspaceId, cursor, limit, sortKey });
}

export async function listMcpServerStatus(
  workspaceId: string,
  cursor?: string | null,
  limit?: number | null,
) {
  return invoke<any>("list_mcp_server_status", { workspaceId, cursor, limit });
}

export async function resumeThread(workspaceId: string, threadId: string) {
  return invoke<any>("resume_thread", { workspaceId, threadId });
}

export async function readThread(workspaceId: string, threadId: string) {
  return invoke<any>("read_thread", { workspaceId, threadId });
}

export async function threadLiveSubscribe(workspaceId: string, threadId: string) {
  return invoke<any>("thread_live_subscribe", { workspaceId, threadId });
}

export async function threadLiveUnsubscribe(workspaceId: string, threadId: string) {
  return invoke<any>("thread_live_unsubscribe", { workspaceId, threadId });
}

export async function archiveThread(workspaceId: string, threadId: string) {
  return invoke<any>("archive_thread", { workspaceId, threadId });
}

export async function setThreadName(
  workspaceId: string,
  threadId: string,
  name: string,
) {
  return invoke<any>("set_thread_name", { workspaceId, threadId, name });
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
  return invoke<any>("account_rate_limits", { workspaceId });
}

export async function getAccountInfo(workspaceId: string) {
  return invoke<any>("account_read", { workspaceId });
}

export async function runCodexLogin(workspaceId: string) {
  return invoke<{ loginId: string; authUrl: string; raw?: unknown }>("codex_login", {
    workspaceId,
  });
}

export async function cancelCodexLogin(workspaceId: string) {
  return invoke<{ canceled: boolean; status?: string; raw?: unknown }>(
    "codex_login_cancel",
    { workspaceId },
  );
}
