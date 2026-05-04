import { invoke } from "../client";

export type ProviderListItem = {
  id: string;
  name: string;
  hasApiKey: boolean;
  defaultModel: string | null;
  isPrimary: boolean;
  isFallback: boolean;
};

export type ProvidersListResult = {
  providers: ProviderListItem[];
  primary: string | null;
  fallback: string | null;
};

export type ProviderStatusResult = {
  id: string;
  available: boolean;
  error: string | null;
};

export async function providersList(): Promise<ProvidersListResult> {
  return invoke<ProvidersListResult>("providers_list");
}

export async function providerStatus(providerId: string): Promise<ProviderStatusResult> {
  return invoke<ProviderStatusResult>("provider_status", { providerId });
}
