// TODO(klynt-integration): replace this mock with the real `@tauri-apps/api/core`
// once Klynt's Rust backend exposes the equivalent commands.
//
// This module is aliased in vite.config.ts so every import of
// `@tauri-apps/api/core` resolves here. To restore the real Tauri API,
// remove the alias entries from vite.config.ts -> resolve.alias.
//
// `invoke` calls log their command name + args and return a generic
// default value chosen to keep the UI from crashing. Per-command shaped
// mocks can be added to `commandMocks` below as features get wired.

const commandMocks: Record<string, (args?: Record<string, unknown>) => unknown> = {
  // TODO(klynt-integration): add per-command mocks here as features come online.
  // Example:
  //   list_workspaces: () => [],
  //   get_app_settings: () => ({ theme: "dark" }),
};

function defaultMockResult(): unknown {
  // null is the safest default — many call sites coalesce with `?? fallback`.
  return null;
}

export async function invoke<T = unknown>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  // eslint-disable-next-line no-console
  console.debug(`[mock invoke] ${cmd}`, args ?? {});
  const handler = commandMocks[cmd];
  const result = handler ? handler(args) : defaultMockResult();
  return result as T;
}

export const convertFileSrc = (filePath: string, _protocol = "asset"): string => {
  // TODO(klynt-integration): implement asset URL conversion when wiring real Tauri.
  return filePath;
};

export class Channel<T = unknown> {
  // TODO(klynt-integration): real Channel posts messages back from Rust.
  onmessage: ((message: T) => void) | null = null;
  toJSON(): string {
    return "__CHANNEL__:mock";
  }
}

export class PluginListener {
  constructor(
    public plugin: string,
    public event: string,
    public channelId: number,
  ) {}
  async unregister(): Promise<void> {
    // no-op
  }
}

export async function addPluginListener<T>(
  _plugin: string,
  _event: string,
  _cb: (payload: T) => void,
): Promise<PluginListener> {
  return new PluginListener("mock", "mock", 0);
}

export async function checkPermissions<T>(): Promise<T> {
  return {} as T;
}

export async function requestPermissions<T>(): Promise<T> {
  return {} as T;
}

export const isTauri = (): boolean => false;
