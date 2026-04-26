export { invoke } from "@tauri-apps/api/core";

export function isMissingTauriInvokeError(error: unknown) {
  return (
    error instanceof TypeError &&
    (error.message.includes("reading 'invoke'") ||
      error.message.includes('reading "invoke"'))
  );
}
