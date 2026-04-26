// TODO(klynt-integration): replace with real `@tauri-apps/api/event` once
// the Rust backend emits matching events. Aliased in vite.config.ts.

export type EventName = string;
export type Event<T> = { event: EventName; id: number; payload: T };
export type EventCallback<T> = (event: Event<T>) => void;
export type UnlistenFn = () => void;

let nextId = 1;

export async function listen<T>(
  event: EventName,
  _handler: EventCallback<T>,
): Promise<UnlistenFn> {
  // eslint-disable-next-line no-console
  console.debug(`[mock listen] ${event}`);
  const id = nextId++;
  return () => {
    // eslint-disable-next-line no-console
    console.debug(`[mock unlisten] ${event} #${id}`);
  };
}

export async function once<T>(
  event: EventName,
  _handler: EventCallback<T>,
): Promise<UnlistenFn> {
  // eslint-disable-next-line no-console
  console.debug(`[mock once] ${event}`);
  return () => {};
}

export async function emit(event: EventName, payload?: unknown): Promise<void> {
  // eslint-disable-next-line no-console
  console.debug(`[mock emit] ${event}`, payload);
}

export async function emitTo(
  target: string,
  event: EventName,
  payload?: unknown,
): Promise<void> {
  // eslint-disable-next-line no-console
  console.debug(`[mock emitTo] ${target} ${event}`, payload);
}

export const TauriEvent = {
  WINDOW_RESIZED: "tauri://resize",
  WINDOW_MOVED: "tauri://move",
  WINDOW_CLOSE_REQUESTED: "tauri://close-requested",
  WINDOW_DESTROYED: "tauri://destroyed",
  WINDOW_FOCUS: "tauri://focus",
  WINDOW_BLUR: "tauri://blur",
  WINDOW_SCALE_FACTOR_CHANGED: "tauri://scale-change",
  WINDOW_THEME_CHANGED: "tauri://theme-changed",
  WINDOW_CREATED: "tauri://window-created",
  WEBVIEW_CREATED: "tauri://webview-created",
  DRAG_ENTER: "tauri://drag-enter",
  DRAG_OVER: "tauri://drag-over",
  DRAG_DROP: "tauri://drag-drop",
  DRAG_LEAVE: "tauri://drag-leave",
} as const;
