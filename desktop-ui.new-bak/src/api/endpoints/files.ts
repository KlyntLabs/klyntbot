import { open, save } from "@tauri-apps/plugin-dialog";
import { invoke } from "../client";

export async function pickWorkspacePath(): Promise<string | null> {
  const selection = await open({ directory: true, multiple: false });
  if (!selection || Array.isArray(selection)) {
    return null;
  }
  return selection;
}

export async function pickWorkspacePaths(): Promise<string[]> {
  const selection = await open({ directory: true, multiple: true });
  if (!selection) {
    return [];
  }
  return Array.isArray(selection) ? selection : [selection];
}

export async function pickImageFiles(): Promise<string[]> {
  const selection = await open({
    multiple: true,
    filters: [
      {
        name: "Images",
        extensions: ["png", "jpg", "jpeg", "gif", "webp", "bmp", "tiff", "tif", "heic", "heif"],
      },
    ],
  });
  if (!selection) {
    return [];
  }
  return Array.isArray(selection) ? selection : [selection];
}

export async function exportMarkdownFile(
  content: string,
  defaultFileName = "plan.md",
): Promise<string | null> {
  const selection = await save({
    title: "Export plan as Markdown",
    defaultPath: defaultFileName,
    filters: [
      {
        name: "Markdown",
        extensions: ["md"],
      },
    ],
  });
  if (!selection) {
    return null;
  }
  await invoke("text_file_write", { path: selection, content });
  return selection;
}

export async function readImageAsDataUrl(path: string): Promise<string> {
  return invoke<string>("image_data_url", { path });
}

export type TextFileResponse = {
  exists: boolean;
  content: string;
  truncated: boolean;
};

export type GlobalAgentsResponse = TextFileResponse;
export type GlobalCodexConfigResponse = TextFileResponse;
export type AgentMdResponse = TextFileResponse;

type FileScope = "workspace" | "global";
type FileKind = "agents" | "config";

export async function fileRead(
  scope: FileScope,
  kind: FileKind,
  workspaceId?: string,
): Promise<TextFileResponse> {
  if (!workspaceId && scope === "workspace") {
    throw new Error("workspaceId required for workspace scope");
  }
  return invoke<TextFileResponse>("workspace_meta_read", {
    workspaceId: workspaceId ?? "",
    scope,
    kind,
  });
}

export async function fileWrite(
  scope: FileScope,
  kind: FileKind,
  content: string,
  workspaceId?: string,
): Promise<void> {
  if (!workspaceId && scope === "workspace") {
    throw new Error("workspaceId required for workspace scope");
  }
  return invoke("workspace_meta_write", {
    workspaceId: workspaceId ?? "",
    scope,
    kind,
    content,
  });
}

export async function readGlobalAgentsMd(): Promise<GlobalAgentsResponse> {
  return invoke<TextFileResponse>("workspace_meta_read", {
    workspaceId: "",
    scope: "global",
    kind: "agents",
  });
}

export async function writeGlobalAgentsMd(content: string): Promise<void> {
  return invoke("workspace_meta_write", {
    workspaceId: "",
    scope: "global",
    kind: "agents",
    content,
  });
}

export async function readGlobalCodexConfigToml(): Promise<GlobalCodexConfigResponse> {
  return invoke<TextFileResponse>("workspace_meta_read", {
    workspaceId: "",
    scope: "global",
    kind: "config",
  });
}

export async function writeGlobalCodexConfigToml(content: string): Promise<void> {
  return invoke("workspace_meta_write", {
    workspaceId: "",
    scope: "global",
    kind: "config",
    content,
  });
}

export async function readAgentMd(workspaceId: string): Promise<AgentMdResponse> {
  return fileRead("workspace", "agents", workspaceId);
}

export async function writeAgentMd(workspaceId: string, content: string): Promise<void> {
  return fileWrite("workspace", "agents", content, workspaceId);
}

export async function getWorkspaceFiles(workspaceId: string) {
  return invoke<string[]>("workspace_files_list", { workspaceId, query: null, limit: null });
}

export async function readWorkspaceFile(
  workspaceId: string,
  path: string,
): Promise<{ content: string; truncated: boolean }> {
  return invoke<{ content: string; truncated: boolean }>("workspace_file_read", {
    workspaceId,
    path,
  });
}

export async function getOpenAppIcon(appName: string): Promise<string | null> {
  return invoke<string | null>("get_open_app_icon", { appName });
}

export async function getCodexConfigPath(): Promise<string> {
  return invoke<string>("get_codex_config_path");
}

function isInlineImageUrl(image: string) {
  return image.startsWith("data:") || image.startsWith("http://") || image.startsWith("https://");
}

export async function convertImagesToDataUrls(images: string[]): Promise<string[]> {
  return Promise.all(
    images.map(async (image) => {
      if (isInlineImageUrl(image)) {
        return image;
      }
      return readImageAsDataUrl(image);
    }),
  );
}
