export { invoke } from "@tauri-apps/api/core";

export function isMissingTauriInvokeError(error: unknown) {
  if (error instanceof TypeError) {
    const msg = error.message;
    if (
      msg.includes("reading 'invoke'") ||
      msg.includes('reading "invoke"')
    ) {
      return true;
    }
  }
  // Tauri returns a plain error string when a command is not registered.
  if (typeof error === "string") {
    return (
      error.includes("Command") && error.includes("not found")
    );
  }
  if (error instanceof Error) {
    const msg = error.message;
    return (
      msg.includes("Command") && msg.includes("not found")
    );
  }
  return false;
}
