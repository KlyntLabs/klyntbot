import { connectWorkspace, getConfigModel, getModelList } from "@services/tauri";
import { useCallback, useMemo } from "react";
import { parseModelListResponse } from "@/features/models/utils/modelListResponse";
import { qk, useTauriQuery } from "@/lib/query";
import type { ModelOption, WorkspaceInfo } from "@/types";

type SettingsDefaultModelsState = {
  models: ModelOption[];
  isLoading: boolean;
  error: string | null;
  connectedWorkspaceCount: number;
};

const EMPTY_STATE: SettingsDefaultModelsState = {
  models: [],
  isLoading: false,
  error: null,
  connectedWorkspaceCount: 0,
};

const CONFIG_MODEL_DESCRIPTION = "Configured in CODEX_HOME/config.toml";

const parseGptVersionScore = (slug: string): number | null => {
  const match = /^gpt-(\d+)(?:\.(\d+))?(?:\.(\d+))?/i.exec(slug.trim());
  if (!match) {
    return null;
  }
  const major = Number(match[1] ?? NaN);
  const minor = Number(match[2] ?? 0);
  const patch = Number(match[3] ?? 0);
  if (!Number.isFinite(major)) {
    return null;
  }
  return major * 1_000_000 + minor * 1_000 + patch;
};

const gptVariantPenalty = (slug: string): number => {
  const match = /^gpt-(\d+(?:\.\d+){0,2})(.*)$/i.exec(slug.trim());
  if (!match) {
    return 1;
  }
  const suffix = match[2] ?? "";
  return suffix.startsWith("-") ? 1 : 0;
};

function compareModelsByLatest(a: ModelOption, b: ModelOption): number {
  const scoreA = parseGptVersionScore(a.model) ?? -1;
  const scoreB = parseGptVersionScore(b.model) ?? -1;
  if (scoreA !== scoreB) {
    return scoreB - scoreA;
  }
  const penaltyA = gptVariantPenalty(a.model);
  const penaltyB = gptVariantPenalty(b.model);
  if (penaltyA !== penaltyB) {
    return penaltyA - penaltyB;
  }
  if (a.isDefault !== b.isDefault) {
    return a.isDefault ? -1 : 1;
  }
  return a.model.localeCompare(b.model);
}

export function useSettingsDefaultModels(projects: WorkspaceInfo[]) {
  const workspaceIds = useMemo(
    () =>
      projects
        .map((p) => p.id)
        .sort()
        .join(","),
    [projects],
  );

  const query = useTauriQuery<SettingsDefaultModelsState>({
    queryKey: qk.settings.defaultModels(workspaceIds),
    queryFn: async () => {
      let connectedWorkspaceCount = 0;
      let lastError: string | null = null;
      const all: ModelOption[] = [];

      for (const project of projects) {
        try {
          await connectWorkspace(project.id);
          connectedWorkspaceCount += 1;
        } catch (e) {
          lastError = String(e);
          continue;
        }
        try {
          const [listRaw, configModel] = await Promise.all([
            getModelList(project.id),
            getConfigModel(project.id),
          ]);
          const list = parseModelListResponse(listRaw);
          if (configModel) {
            all.push({
              id: configModel,
              model: configModel,
              displayName: configModel,
              description: CONFIG_MODEL_DESCRIPTION,
              provider: null,
              supportedReasoningEfforts: [],
              defaultReasoningEffort: null,
              isDefault: true,
            });
          }
          for (const m of list) all.push(m);
        } catch (e) {
          lastError = String(e);
        }
      }

      const dedup = new Map<string, ModelOption>();
      for (const m of all) {
        if (!dedup.has(m.id)) dedup.set(m.id, m);
      }
      const models = Array.from(dedup.values()).sort(compareModelsByLatest);

      return {
        models,
        isLoading: false,
        error: lastError,
        connectedWorkspaceCount,
      };
    },
    fallback: EMPTY_STATE,
  });

  const refresh = useCallback(async () => {
    await query.refetch();
  }, [query]);

  return {
    models: query.data.models,
    isLoading: query.isLoading,
    error: query.data.error,
    connectedWorkspaceCount: query.data.connectedWorkspaceCount,
    refresh,
  };
}
