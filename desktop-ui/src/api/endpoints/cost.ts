import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type CostUpdate = {
  threadId: string | null;
  provider: string;
  promptTokensDelta: number;
  completionTokensDelta: number;
  usdDelta: number;
  threadTotalUsd: number | null;
  ceilingBreached: boolean;
};

export function subscribeCostUpdates(handler: (update: CostUpdate) => void): Promise<UnlistenFn> {
  return listen<CostUpdate>("agent:cost_update", (event) => {
    handler(event.payload);
  });
}
