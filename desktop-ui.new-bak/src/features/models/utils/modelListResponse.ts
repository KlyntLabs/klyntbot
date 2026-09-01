import type { ModelOption } from "@/types";
import { deriveProviderFromModel } from "./deriveProvider";

export function normalizeEffortValue(value: unknown): string | null {
  if (typeof value !== "string") {
    return null;
  }
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function extractModelItems(response: unknown): unknown[] {
  // The new `model_list` IPC returns a flat array directly.
  if (Array.isArray(response)) {
    return response;
  }
  if (!response || typeof response !== "object") {
    return [];
  }

  const record = response as Record<string, unknown>;
  const result =
    record.result && typeof record.result === "object"
      ? (record.result as Record<string, unknown>)
      : null;

  const resultData = result?.data;
  if (Array.isArray(resultData)) {
    return resultData;
  }

  const topLevelData = record.data;
  if (Array.isArray(topLevelData)) {
    return topLevelData;
  }

  return [];
}

function parseReasoningEfforts(
  item: Record<string, unknown>,
): ModelOption["supportedReasoningEfforts"] {
  const camel = item.supportedReasoningEfforts;
  if (Array.isArray(camel)) {
    return camel
      .map((effort) => {
        if (!effort || typeof effort !== "object") {
          return null;
        }
        const entry = effort as Record<string, unknown>;
        return {
          reasoningEffort: String(entry.reasoningEffort ?? entry.reasoning_effort ?? ""),
          description: String(entry.description ?? ""),
        };
      })
      .filter(
        (effort): effort is { reasoningEffort: string; description: string } => effort !== null,
      );
  }

  const snake = item.supported_reasoning_efforts;
  if (Array.isArray(snake)) {
    return snake
      .map((effort) => {
        if (!effort || typeof effort !== "object") {
          return null;
        }
        const entry = effort as Record<string, unknown>;
        return {
          reasoningEffort: String(entry.reasoningEffort ?? entry.reasoning_effort ?? ""),
          description: String(entry.description ?? ""),
        };
      })
      .filter(
        (effort): effort is { reasoningEffort: string; description: string } => effort !== null,
      );
  }

  return [];
}

export function parseModelListResponse(response: unknown): ModelOption[] {
  const items = extractModelItems(response);

  return items
    .map((item) => {
      if (!item || typeof item !== "object") {
        return null;
      }
      const record = item as Record<string, unknown>;
      const modelSlug = String(record.model ?? record.id ?? "");
      const rawDisplayName = String(record.displayName || record.display_name || "");
      const displayName = rawDisplayName.trim().length > 0 ? rawDisplayName : modelSlug;
      // Trust the backend-tagged config-provider key when present
      // (model_list IPC); fall back to keyword-based brand derivation
      // for the legacy shape (older `model_list` responses).
      const backendProvider =
        typeof record.provider === "string" && record.provider.trim().length > 0
          ? record.provider
          : null;
      const supportsReasoning = Boolean(
        record.supportsReasoning ?? record.supports_reasoning ?? false,
      );
      // Synthesize a single reasoning effort entry when the backend
      // tells us reasoning is supported but didn't enumerate efforts —
      // keeps the FE's effort selector enabled.
      let supportedEfforts = parseReasoningEfforts(record);
      if (supportedEfforts.length === 0 && supportsReasoning) {
        supportedEfforts = [{ reasoningEffort: "default", description: "Extended thinking" }];
      }
      const brand =
        typeof record.brand === "string" && record.brand.trim().length > 0
          ? record.brand
          : (backendProvider ?? deriveProviderFromModel(modelSlug));
      const built: ModelOption = {
        id: String(record.id ?? record.model ?? ""),
        model: modelSlug,
        displayName,
        description: String(record.description ?? ""),
        provider: backendProvider ?? deriveProviderFromModel(modelSlug),
        brand: brand ?? null,
        supportedReasoningEfforts: supportedEfforts,
        defaultReasoningEffort: normalizeEffortValue(
          record.defaultReasoningEffort ?? record.default_reasoning_effort,
        ),
        isDefault: Boolean(record.isDefault ?? record.is_default ?? false),
      };
      return built;
    })
    .filter((model): model is ModelOption => model !== null);
}
