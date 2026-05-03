import { invoke } from "../client";

export type ProviderListItem = {
  id: string;
  displayName: string;
  hasApiKey: boolean;
  defaultModel: string | null;
};

export async function providersList(): Promise<ProviderListItem[]> {
  return invoke<ProviderListItem[]>("providers_list");
}

export async function providerStatus(providerId: string): Promise<{ ok: boolean; message: string }> {
  return invoke<{ ok: boolean; message: string }>("provider_status", { providerId });
}
