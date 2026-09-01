import { ipc } from "@/utils/tauri-bridge";

export async function showError(message: string, err: unknown): Promise<void> {
  const detail = err instanceof Error ? err.message : String(err);
  console.error(message, err);
  try {
    await ipc("show_status_badge", {
      text: `${message} ${detail}`.slice(0, 80),
      kind: "error",
      durationMs: 2400,
    });
  } catch {
    // status badge IPC failed — already logged above, no further user surface
  }
}
