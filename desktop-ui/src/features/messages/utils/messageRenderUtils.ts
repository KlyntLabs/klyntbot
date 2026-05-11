import { convertFileSrc } from "@tauri-apps/api/core";
import type { ConversationItem } from "@/types";

export type ToolSummary = {
  label: string;
  value?: string;
  detail?: string;
  output?: string;
};

export type StatusTone = "completed" | "processing" | "failed" | "unknown";

export type ParsedReasoning = {
  summaryTitle: string;
  bodyText: string;
  hasBody: boolean;
  workingLabel: string | null;
};

export type MessageImage = {
  src: string;
  label: string;
};

export type ToolFamily =
  | "filesystem"
  | "shell"
  | "search"
  | "web"
  | "domain"
  | "agent"
  | "mcp"
  | "system"
  | "approval";

export type ToolRowDescriptor = {
  family: ToolFamily;
  /** Display name in the header — capitalised, no trailing colon. */
  name: string;
  /** Primary argument shown after the name (path, command, query, action verb). */
  arg: string;
  /** Optional right-side meta fragments joined with " · " when rendered. */
  meta: string[];
};

export const SCROLL_THRESHOLD_PX = 120;
export const MAX_COMMAND_OUTPUT_LINES = 200;

export function basename(path: string) {
  if (!path) {
    return "";
  }
  const normalized = path.replace(/\\/g, "/");
  const parts = normalized.split("/").filter(Boolean);
  return parts.length ? parts[parts.length - 1] : path;
}

const _parseToolArgsCache = new Map<string, Record<string, unknown> | null>();
function parseToolArgs(detail: string) {
  if (!detail) {
    return null;
  }
  const cached = _parseToolArgsCache.get(detail);
  if (cached !== undefined) {
    return cached;
  }
  try {
    const parsed = JSON.parse(detail) as Record<string, unknown>;
    _parseToolArgsCache.set(detail, parsed);
    return parsed;
  } catch {
    _parseToolArgsCache.set(detail, null);
    return null;
  }
}

function firstStringField(source: Record<string, unknown> | null, keys: string[]) {
  if (!source) {
    return "";
  }
  for (const key of keys) {
    const value = source[key];
    if (typeof value === "string" && value.trim()) {
      return value.trim();
    }
  }
  return "";
}

function formatCollabAgentLabel(agent: { threadId: string; nickname?: string; role?: string }) {
  const nickname = agent.nickname?.trim();
  const role = agent.role?.trim();
  if (nickname && role) {
    return `${nickname} [${role}]`;
  }
  if (nickname) {
    return nickname;
  }
  if (role) {
    return `${agent.threadId} [${role}]`;
  }
  return agent.threadId;
}

function summarizeCollabLabel(title: string, status?: string) {
  const tool = title
    .replace(/^collab:\s*/i, "")
    .trim()
    .toLowerCase();
  const tone = statusToneFromText(status);
  if (tool.includes("wait")) {
    return tone === "processing" ? "waiting for" : "waited for";
  }
  if (tool.includes("resume")) {
    return tone === "processing" ? "resuming" : "resumed";
  }
  if (tool.includes("close")) {
    return tone === "processing" ? "closing" : "closed";
  }
  if (tool.includes("spawn")) {
    return tone === "processing" ? "spawning" : "spawned";
  }
  if (tool.includes("send") || tool.includes("interaction")) {
    return tone === "processing" ? "sending to" : "sent to";
  }
  return "sub-agent";
}

function summarizeCollabReceiver(item: Extract<ConversationItem, { kind: "tool" }>) {
  const receivers =
    item.collabReceivers && item.collabReceivers.length > 0
      ? item.collabReceivers
      : item.collabReceiver
        ? [item.collabReceiver]
        : [];
  if (receivers.length === 0) {
    return item.title || "";
  }
  if (receivers.length === 1) {
    return formatCollabAgentLabel(receivers[0]);
  }
  return `${formatCollabAgentLabel(receivers[0])} +${receivers.length - 1}`;
}

export function toolNameFromTitle(title: string) {
  if (!title.toLowerCase().startsWith("tool:")) {
    return "";
  }
  const [, toolPart = ""] = title.split(":");
  const segments = toolPart.split("/").map((segment) => segment.trim());
  return segments.length ? segments[segments.length - 1] : "";
}

function sanitizeReasoningTitle(title: string) {
  return title
    .replace(/[`*_~]/g, "")
    .replace(/\[(.*?)\]\(.*?\)/g, "$1")
    .trim();
}

export function parseReasoning(
  item: Extract<ConversationItem, { kind: "reasoning" }>,
): ParsedReasoning {
  const summary = item.summary ?? "";
  const content = item.content ?? "";
  const hasSummary = summary.trim().length > 0;
  const titleSource = hasSummary ? summary : content;
  const titleLines = titleSource.split("\n");
  const trimmedLines = titleLines.map((line) => line.trim());
  const titleLineIndex = trimmedLines.findIndex(Boolean);
  const rawTitle = titleLineIndex >= 0 ? trimmedLines[titleLineIndex] : "";
  const cleanTitle = sanitizeReasoningTitle(rawTitle);
  const summaryTitle = cleanTitle
    ? cleanTitle.length > 80
      ? `${cleanTitle.slice(0, 80)}…`
      : cleanTitle
    : "Reasoning";
  const summaryLines = summary.split("\n");
  const contentLines = content.split("\n");
  const summaryBody =
    hasSummary && titleLineIndex >= 0
      ? summaryLines
          .filter((_, index) => index !== titleLineIndex)
          .join("\n")
          .trim()
      : "";
  const contentBody = hasSummary
    ? content.trim()
    : titleLineIndex >= 0
      ? contentLines
          .filter((_, index) => index !== titleLineIndex)
          .join("\n")
          .trim()
      : content.trim();
  const bodyParts = [summaryBody, contentBody].filter(Boolean);
  const bodyText = bodyParts.join("\n\n").trim();
  const hasBody = bodyText.length > 0;
  const hasAnyText = titleSource.trim().length > 0;
  const workingLabel = hasAnyText ? summaryTitle : null;
  return {
    summaryTitle,
    bodyText,
    hasBody,
    workingLabel,
  };
}

export function normalizeMessageImageSrc(path: string) {
  if (!path) {
    return "";
  }
  if (path.startsWith("data:") || path.startsWith("http://") || path.startsWith("https://")) {
    return path;
  }
  if (path.startsWith("file://")) {
    return path;
  }
  try {
    return convertFileSrc(path);
  } catch {
    return "";
  }
}

export function cleanCommandText(commandText: string) {
  if (!commandText) {
    return "";
  }
  const trimmed = commandText.trim();
  const shellMatch = trimmed.match(
    /^(?:\/\S+\/)?(?:bash|zsh|sh|fish)(?:\.exe)?\s+-lc\s+(['"])([\s\S]+)\1$/,
  );
  const inner = shellMatch ? shellMatch[2] : trimmed;
  const cdMatch = inner.match(/^\s*cd\s+[^&;]+(?:\s*&&\s*|\s*;\s*)([\s\S]+)$/i);
  const stripped = cdMatch ? cdMatch[1] : inner;
  return stripped.trim();
}

export function buildToolSummary(
  item: Extract<ConversationItem, { kind: "tool" }>,
  commandText: string,
): ToolSummary {
  if (item.toolType === "commandExecution") {
    const cleanedCommand = cleanCommandText(commandText);
    return {
      label: "command",
      value: cleanedCommand || "Command",
      detail: "",
      output: item.output || "",
    };
  }

  if (item.toolType === "webSearch") {
    return {
      label: statusToneFromText(item.status) === "processing" ? "searching" : "searched",
      value: item.detail || "the web",
    };
  }

  if (item.toolType === "imageView") {
    const file = basename(item.detail || "");
    return {
      label: "read",
      value: file || "image",
    };
  }

  if (item.toolType === "hook") {
    return {
      label: "hook",
      value: item.title.replace(/^Hook:\s*/i, "").trim() || item.title || "hook",
      detail: item.detail || "",
      output: item.output || "",
    };
  }

  if (item.toolType === "collabToolCall") {
    return {
      label: summarizeCollabLabel(item.title, item.status),
      value: summarizeCollabReceiver(item),
      detail: item.detail || "",
      output: item.output || "",
    };
  }

  if (item.toolType === "mcpToolCall") {
    const toolName = toolNameFromTitle(item.title);
    const args = parseToolArgs(item.detail);
    if (toolName.toLowerCase().includes("search")) {
      return {
        label: statusToneFromText(item.status) === "processing" ? "searching" : "searched",
        value: firstStringField(args, ["query", "pattern", "text"]) || item.detail,
      };
    }
    if (toolName.toLowerCase().includes("read")) {
      const targetPath = firstStringField(args, ["path", "file", "filename"]) || item.detail;
      return {
        label: "read",
        value: basename(targetPath),
        detail: targetPath && targetPath !== basename(targetPath) ? targetPath : "",
      };
    }
    if (toolName) {
      return {
        label: "tool",
        value: toolName,
        detail: item.detail || "",
      };
    }
  }

  return {
    label: "tool",
    value: item.title || "",
    detail: item.detail || "",
    output: item.output || "",
  };
}

export function formatDurationMs(durationMs: number) {
  const durationSeconds = Math.max(0, Math.floor(durationMs / 1000));
  const durationMinutes = Math.floor(durationSeconds / 60);
  const durationRemainder = durationSeconds % 60;
  return `${durationMinutes}:${String(durationRemainder).padStart(2, "0")}`;
}

export function statusToneFromText(status?: string): StatusTone {
  if (!status) {
    return "unknown";
  }
  const normalized = status.toLowerCase();
  if (/(fail|error)/.test(normalized)) {
    return "failed";
  }
  if (/(pending|running|processing|started|in[_\s-]?progress)/.test(normalized)) {
    return "processing";
  }
  if (/(complete|completed|success|done)/.test(normalized)) {
    return "completed";
  }
  return "unknown";
}

export function toolStatusTone(
  item: Extract<ConversationItem, { kind: "tool" }>,
  hasChanges: boolean,
): StatusTone {
  const fromStatus = statusToneFromText(item.status);
  if (fromStatus !== "unknown") {
    return fromStatus;
  }
  if (item.output || hasChanges) {
    return "completed";
  }
  return "processing";
}

export function formatToolStatusLabel(item: Extract<ConversationItem, { kind: "tool" }>) {
  if (item.toolType !== "hook") {
    return "";
  }
  const parts: string[] = [];
  const status = (item.status ?? "").trim().toLowerCase();
  if (status) {
    parts.push(status.replace(/[_-]+/g, " "));
  }
  if (typeof item.durationMs === "number" && Number.isFinite(item.durationMs)) {
    parts.push(formatDurationMs(item.durationMs));
  }
  return parts.join(" • ");
}

export type PlanFollowupState = {
  shouldShow: boolean;
  planItemId: string | null;
};

export function computePlanFollowupState({
  threadId,
  items,
  isThinking,
  hasVisibleUserInputRequest,
}: {
  threadId: string | null;
  items: ConversationItem[];
  isThinking: boolean;
  hasVisibleUserInputRequest: boolean;
}): PlanFollowupState {
  if (!threadId) {
    return { shouldShow: false, planItemId: null };
  }
  if (hasVisibleUserInputRequest) {
    return { shouldShow: false, planItemId: null };
  }

  let planIndex = -1;
  let planItem: Extract<ConversationItem, { kind: "tool" }> | null = null;
  for (let index = items.length - 1; index >= 0; index -= 1) {
    const item = items[index];
    if (item.kind === "tool" && item.toolType === "plan") {
      planIndex = index;
      planItem = item;
      break;
    }
  }

  if (!planItem) {
    return { shouldShow: false, planItemId: null };
  }

  const planItemId = planItem.id;

  if (!(planItem.output ?? "").trim()) {
    return { shouldShow: false, planItemId };
  }

  const planTone = toolStatusTone(planItem, false);
  if (planTone === "failed") {
    return { shouldShow: false, planItemId };
  }

  // Some backends stream plan output deltas without a final status update. As
  // soon as the turn stops thinking, treat the latest plan output as ready.
  if (isThinking && planTone !== "completed") {
    return { shouldShow: false, planItemId };
  }

  for (let index = planIndex + 1; index < items.length; index += 1) {
    const item = items[index];
    if (item.kind === "message" && item.role === "user") {
      return { shouldShow: false, planItemId };
    }
  }

  return { shouldShow: true, planItemId };
}

export function scrollKeyForItems(items: ConversationItem[]) {
  if (!items.length) {
    return "empty";
  }
  const last = items[items.length - 1];
  switch (last.kind) {
    case "message":
      return `${last.id}-${last.text.length}`;
    case "userInput":
      return `${last.id}-${last.status}-${last.questions.length}`;
    case "reasoning":
      return `${last.id}-${last.summary.length}-${last.content.length}`;
    case "explore":
      return `${last.id}-${last.status}-${last.entries.length}`;
    case "tool":
      return `${last.id}-${last.status ?? ""}-${last.output?.length ?? 0}`;
    case "diff":
      return `${last.id}-${last.status ?? ""}-${last.diff.length}`;
    case "review":
      return `${last.id}-${last.state}-${last.text.length}`;
    case "approval":
      return `${last.id}-${last.status}`;
    case "recall":
      return `${last.id}-${last.coverage_score}`;
    case "dead_end_warning":
      return `${last.id}-${last.confidence}`;
    default: {
      const _exhaustive: never = last;
      return _exhaustive;
    }
  }
}

export function exploreKindLabel(
  kind: Extract<ConversationItem, { kind: "explore" }>["entries"][number]["kind"],
) {
  return kind[0].toUpperCase() + kind.slice(1);
}

const SHELL_SEARCH_TOOLS = ["grep", "glob", "rg", "ripgrep", "fd", "find"];

function classifyShellCommand(command: string): "search" | "shell" {
  const head = command.trim().split(/\s+/)[0]?.toLowerCase() ?? "";
  return SHELL_SEARCH_TOOLS.includes(head) ? "search" : "shell";
}

function formatDurationCompact(ms: number | null | undefined): string | null {
  if (typeof ms !== "number" || !Number.isFinite(ms) || ms <= 0) return null;
  if (ms < 1000) return `${ms}ms`;
  const seconds = ms / 1000;
  if (seconds < 60) return `${seconds.toFixed(seconds < 10 ? 1 : 0)}s`;
  const minutes = Math.floor(seconds / 60);
  const rem = Math.round(seconds % 60);
  return `${minutes}m${rem.toString().padStart(2, "0")}s`;
}

function summarizeDiffStats(diff: string): { added: number; removed: number } {
  let added = 0;
  let removed = 0;
  for (const line of diff.split("\n")) {
    if (line.startsWith("+") && !line.startsWith("+++")) added += 1;
    else if (line.startsWith("-") && !line.startsWith("---")) removed += 1;
  }
  return { added, removed };
}

function fileChangeName(kind: string | undefined): "Read" | "Write" | "Edit" | "Patch" {
  switch (kind) {
    case "read":
      return "Read";
    case "add":
    case "write":
      return "Write";
    case "apply_patch":
    case "notebook_edit":
      return "Patch";
    default:
      return "Edit";
  }
}

function parseMcpTitle(title: string): { server: string; tool: string } {
  const match = title.match(/^Tool:\s*([^\s/]+)\s*(?:\/\s*(.+))?$/i);
  if (!match) return { server: "", tool: title };
  return { server: match[1] ?? "", tool: (match[2] ?? "").trim() };
}

const KLYNTBOT_MCP_SERVERS = new Set(["klyntbot", "klynt", "klyntcoach"]);

function capitalize(input: string): string {
  if (!input) return input;
  return input[0].toUpperCase() + input.slice(1);
}

function summarizeMcpArgs(detail: string): string[] {
  const args = parseToolArgs(detail);
  if (!args) return [];
  const interesting: string[] = [];
  for (const key of ["title", "name", "query", "path", "id"]) {
    const v = args[key];
    if (typeof v === "string" && v.trim()) {
      interesting.push(`${key}=${v.length > 40 ? `${v.slice(0, 40)}…` : v}`);
      break;
    }
  }
  return interesting;
}

export function toolRowDescriptor(
  item: Extract<ConversationItem, { kind: "tool" }>,
): ToolRowDescriptor {
  if (item.toolType === "commandExecution") {
    const command = item.title.replace(/^Command:\s*/i, "").trim() || "command";
    const family = classifyShellCommand(command);
    const meta: string[] = [];
    const dur = formatDurationCompact(item.durationMs ?? null);
    if (dur) meta.push(dur);
    return {
      family,
      name: family === "search" ? "Grep" : "Bash",
      arg: command,
      meta,
    };
  }
  if (item.toolType === "fileChange") {
    const change = item.changes?.[0];
    const path = change?.path ?? item.detail ?? "";
    const name = fileChangeName(change?.kind);
    const meta: string[] = [];
    if (change?.diff) {
      const { added, removed } = summarizeDiffStats(change.diff);
      if (name === "Write" && added > 0) meta.push(`+${added}`);
      else if (name !== "Read" && (added > 0 || removed > 0)) meta.push(`+${added} −${removed}`);
    }
    if ((item.changes?.length ?? 0) > 1) {
      meta.push(`${item.changes!.length} files`);
    }
    return { family: "filesystem", name, arg: path, meta };
  }
  if (item.toolType === "webSearch") {
    return {
      family: "web",
      name: "WebSearch",
      arg: item.detail || "",
      meta: [],
    };
  }
  if (item.toolType === "mcpToolCall") {
    const { server, tool } = parseMcpTitle(item.title);
    const isKlyntbot = KLYNTBOT_MCP_SERVERS.has(server.toLowerCase());
    const args = parseToolArgs(item.detail);
    if (isKlyntbot) {
      const action = (args && typeof args.action === "string" ? args.action : "") || tool || "";
      return {
        family: "domain",
        name: capitalize(tool || "Tool"),
        arg: action,
        meta: summarizeMcpArgs(item.detail),
      };
    }
    return {
      family: "mcp",
      name: server || "mcp",
      arg: tool || "",
      meta: summarizeMcpArgs(item.detail),
    };
  }
  if (item.toolType === "collabToolCall" || item.toolType === "collabAgentToolCall") {
    return {
      family: "agent",
      name: "Agent",
      arg: item.detail || item.title || "",
      meta: formatDurationCompact(item.durationMs ?? null)
        ? [formatDurationCompact(item.durationMs ?? null)!]
        : [],
    };
  }
  if (item.toolType === "hook") {
    return {
      family: "system",
      name: "Hook",
      arg: item.title.replace(/^Hook:\s*/i, "").trim() || "",
      meta: item.detail ? [item.detail] : [],
    };
  }
  if (item.toolType === "contextCompaction") {
    return { family: "system", name: "Context", arg: "compacted", meta: [] };
  }
  if (item.toolType === "imageView") {
    return { family: "system", name: "Image", arg: item.detail || "", meta: [] };
  }
  if (item.toolType === "plan") {
    return { family: "domain", name: "Plan", arg: "", meta: [] };
  }
  // Generic fallback for raw tool names emitted by the coding bridge —
  // `read`, `list_dir`, `glob`, `tool_search`, `bash`, etc. Render as
  // `<tool>: <primary arg>` by reaching into the args JSON (stored in
  // `item.detail`) for the most-meaningful field. Without this branch the
  // row reads as bare `read:` with no path, which is what users were seeing.
  const fallbackArgs = parseToolArgs(item.detail);
  const fallbackPrimary = firstStringField(fallbackArgs, [
    // File-ish tools
    "path",
    "file_path",
    "filename",
    "file",
    // Directory-ish tools
    "directory",
    "dir",
    "target_directory",
    // Search-ish tools
    "pattern",
    "glob",
    "query",
    "q",
    "search",
    "search_query",
    // Shell / network
    "command",
    "cmd",
    "url",
  ]);
  // Last-resort: take the first string-valued field in the args object.
  const fallbackArg =
    fallbackPrimary ||
    (fallbackArgs
      ? ((Object.values(fallbackArgs).find((v) => typeof v === "string" && v.trim().length > 0) as
          | string
          | undefined) ?? "")
      : item.detail);
  const truncatedArg = fallbackArg.length > 80 ? `${fallbackArg.slice(0, 80)}…` : fallbackArg;
  return {
    family: "system",
    name: item.title || "Tool",
    arg: truncatedArg,
    meta: [],
  };
}
