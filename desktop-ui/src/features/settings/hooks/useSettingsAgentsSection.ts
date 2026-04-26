import { useCallback, useState } from "react";
import type { AgentsSettings, GeneratedAgentConfiguration } from "@services/tauri";
import type { ModelOption, WorkspaceInfo } from "@/types";
import {
	connectWorkspace,
	createAgent,
	deleteAgent,
	generateAgentDescription,
	getAgentsSettings,
	readAgentConfigToml,
	setAgentsCoreSettings,
	updateAgent,
	writeAgentConfigToml,
} from "@services/tauri";
import { qk, useTauriMutation, useTauriQuery } from "@/lib/query";
import { useSettingsDefaultModels } from "./useSettingsDefaultModels";

type UseSettingsAgentsSectionArgs = {
	projects: WorkspaceInfo[];
};

export type SettingsAgentsSectionProps = {
	settings: AgentsSettings | null;
	isLoading: boolean;
	isUpdatingCore: boolean;
	creatingAgent: boolean;
	updatingAgentName: string | null;
	deletingAgentName: string | null;
	readingConfigAgentName: string | null;
	writingConfigAgentName: string | null;
	error: string | null;
	onRefresh: () => void;
	onSetMultiAgentEnabled: (enabled: boolean) => Promise<boolean>;
	onSetMaxThreads: (maxThreads: number) => Promise<boolean>;
	onSetMaxDepth: (maxDepth: number) => Promise<boolean>;
	onCreateAgent: (input: {
		name: string;
		description?: string | null;
		developerInstructions?: string | null;
		template?: "blank";
		model?: string | null;
		reasoningEffort?: string | null;
	}) => Promise<boolean>;
	onUpdateAgent: (input: {
		originalName: string;
		name: string;
		description?: string | null;
		developerInstructions?: string | null;
		renameManagedFile?: boolean;
	}) => Promise<boolean>;
	onDeleteAgent: (input: {
		name: string;
		deleteManagedFile?: boolean;
	}) => Promise<boolean>;
	onReadAgentConfig: (agentName: string) => Promise<string | null>;
	onWriteAgentConfig: (agentName: string, content: string) => Promise<boolean>;
	createDescriptionGenerating: boolean;
	editDescriptionGenerating: boolean;
	onGenerateCreateDescription: (seed: {
		name?: string;
		description: string;
		developerInstructions: string;
	}) => Promise<GeneratedAgentConfiguration | null>;
	onGenerateEditDescription: (seed: {
		name?: string;
		description: string;
		developerInstructions: string;
	}) => Promise<GeneratedAgentConfiguration | null>;
	modelOptions: ModelOption[];
	modelOptionsLoading: boolean;
	modelOptionsError: string | null;
};

const toErrorMessage = (value: unknown, fallback: string): string => {
	if (value instanceof Error) {
		return value.message;
	}
	if (typeof value === "string" && value.trim().length > 0) {
		return value;
	}
	return fallback;
};

export const useSettingsAgentsSection = ({
	projects,
}: UseSettingsAgentsSectionArgs): SettingsAgentsSectionProps => {
	const sourceWorkspaceId = projects[0]?.id ?? null;
	const sourceWorkspaceName = projects[0]?.name ?? null;
	const sourceWorkspaceConnected = projects[0]?.connected ?? false;
	const {
		models: modelOptions,
		isLoading: modelOptionsLoading,
		error: modelOptionsError,
	} = useSettingsDefaultModels(projects);

	const settingsQuery = useTauriQuery<AgentsSettings | null>({
		queryKey: qk.agents.settings(),
		queryFn: () => getAgentsSettings(),
		fallback: null,
	});

	const setCore = useTauriMutation<
		AgentsSettings,
		{ multiAgentEnabled: boolean; maxThreads: number; maxDepth: number }
	>({
		mutationFn: (input) =>
			setAgentsCoreSettings({
				multiAgentEnabled: input.multiAgentEnabled,
				maxThreads: input.maxThreads,
				maxDepth: input.maxDepth,
			}),
		invalidates: [qk.agents.settings()],
	});

	const create = useTauriMutation<AgentsSettings, Parameters<typeof createAgent>[0]>({
		mutationFn: (input) => createAgent(input),
		invalidates: [qk.agents.settings()],
	});

	const update = useTauriMutation<AgentsSettings, Parameters<typeof updateAgent>[0]>({
		mutationFn: (input) => updateAgent(input),
		invalidates: [qk.agents.settings()],
	});

	const remove = useTauriMutation<AgentsSettings, Parameters<typeof deleteAgent>[0]>({
		mutationFn: (input) => deleteAgent(input),
		invalidates: [qk.agents.settings()],
	});

	const readToml = useTauriMutation<string | null, { name: string }>({
		mutationFn: (input) => readAgentConfigToml(input.name),
		invalidates: [],
	});

	const writeToml = useTauriMutation<
		void,
		{ name: string; content: string }
	>({
		mutationFn: (input) => writeAgentConfigToml(input.name, input.content),
		invalidates: [qk.agents.settings()],
	});

	const generateDescription = useTauriMutation<
		GeneratedAgentConfiguration | null,
		{
			target: "create" | "edit";
			seed: { name?: string; description: string; developerInstructions: string };
		}
	>({
		mutationFn: async (input) => {
			const nameSeed = input.seed.name?.trim() ?? "";
			const descriptionSeed = input.seed.description.trim();
			const developerInstructionsSeed = input.seed.developerInstructions.trim();
			const promptSeed = [
				nameSeed ? `Agent name:\n${nameSeed}` : null,
				descriptionSeed ? `Description seed:\n${descriptionSeed}` : null,
				developerInstructionsSeed
					? `Developer instructions seed:\n${developerInstructionsSeed}`
					: null,
			]
				.filter((value): value is string => Boolean(value))
				.join("\n\n")
				.trim();
			const effectivePromptSeed =
				promptSeed.length > 0
					? promptSeed
					: "Create a practical custom coding agent configuration.";
			if (!sourceWorkspaceId) throw new Error("No workspace");
			if (!sourceWorkspaceConnected) {
				await connectWorkspace(sourceWorkspaceId);
			}
			const generated = await generateAgentDescription(
				sourceWorkspaceId,
				effectivePromptSeed,
			);
			const nextDescription = generated.description.trim();
			const nextInstructions = generated.developerInstructions.trim();
			if (!nextDescription && !nextInstructions) {
				throw new Error("Generated agent configuration was empty.");
			}
			return {
				description: nextDescription,
				developerInstructions: nextInstructions,
			};
		},
		invalidates: [],
	});

	const [updatingAgentName, setUpdatingAgentName] = useState<string | null>(null);
	const [deletingAgentName, setDeletingAgentName] = useState<string | null>(null);
	const [readingConfigAgentName, setReadingConfigAgentName] = useState<
		string | null
	>(null);
	const [writingConfigAgentName, setWritingConfigAgentName] = useState<
		string | null
	>(null);
	const [generatingDescriptionTarget, setGeneratingDescriptionTarget] =
		useState<"create" | "edit" | null>(null);
	const [error, setError] = useState<string | null>(null);

	const onUpdateAgent = useCallback(
		async (input: Parameters<typeof update.mutate>[0]) => {
			setUpdatingAgentName(input.originalName);
			setError(null);
			try {
				await update.mutate(input);
				return true;
			} catch (e) {
				setError(toErrorMessage(e, "Unable to update agent."));
				return false;
			} finally {
				setUpdatingAgentName(null);
			}
		},
		[update],
	);

	const onDeleteAgent = useCallback(
		async (input: Parameters<typeof remove.mutate>[0]) => {
			setDeletingAgentName(input.name);
			setError(null);
			try {
				await remove.mutate(input);
				return true;
			} catch (e) {
				setError(toErrorMessage(e, "Unable to delete agent."));
				return false;
			} finally {
				setDeletingAgentName(null);
			}
		},
		[remove],
	);

	const onReadAgentConfig = useCallback(
		async (agentName: string): Promise<string | null> => {
			setReadingConfigAgentName(agentName);
			setError(null);
			try {
				return await readToml.mutate({ name: agentName });
			} catch (e) {
				setError(toErrorMessage(e, "Unable to read agent config file."));
				return null;
			} finally {
				setReadingConfigAgentName(null);
			}
		},
		[readToml],
	);

	const onWriteAgentConfig = useCallback(
		async (agentName: string, content: string): Promise<boolean> => {
			setWritingConfigAgentName(agentName);
			setError(null);
			try {
				await writeToml.mutate({ name: agentName, content });
				return true;
			} catch (e) {
				setError(toErrorMessage(e, "Unable to write agent config file."));
				return false;
			} finally {
				setWritingConfigAgentName(null);
			}
		},
		[writeToml],
	);

	const onRefresh = useCallback(() => {
		settingsQuery.refetch();
	}, [settingsQuery]);

	return {
		settings: settingsQuery.data,
		isLoading: settingsQuery.isLoading,
		isUpdatingCore: setCore.isLoading,
		creatingAgent: create.isLoading,
		updatingAgentName,
		deletingAgentName,
		readingConfigAgentName,
		writingConfigAgentName,
		error,
		onRefresh,
		onSetMultiAgentEnabled: async (enabled: boolean) => {
			const current = settingsQuery.data;
			if (!current) return false;
			setError(null);
			try {
				await setCore.mutate({
					multiAgentEnabled: enabled,
					maxThreads: current.maxThreads,
					maxDepth: current.maxDepth,
				});
				return true;
			} catch (e) {
				setError(toErrorMessage(e, "Unable to update agents core settings."));
				return false;
			}
		},
		onSetMaxThreads: async (maxThreads: number) => {
			const current = settingsQuery.data;
			if (!current) return false;
			setError(null);
			try {
				await setCore.mutate({
					multiAgentEnabled: current.multiAgentEnabled,
					maxThreads,
					maxDepth: current.maxDepth,
				});
				return true;
			} catch (e) {
				setError(toErrorMessage(e, "Unable to update max threads."));
				return false;
			}
		},
		onSetMaxDepth: async (maxDepth: number) => {
			const current = settingsQuery.data;
			if (!current) return false;
			setError(null);
			try {
				await setCore.mutate({
					multiAgentEnabled: current.multiAgentEnabled,
					maxThreads: current.maxThreads,
					maxDepth,
				});
				return true;
			} catch (e) {
				setError(toErrorMessage(e, "Unable to update max depth."));
				return false;
			}
		},
		onCreateAgent: async (input) => {
			setError(null);
			try {
				await create.mutate(input);
				return true;
			} catch (e) {
				setError(toErrorMessage(e, "Unable to create agent."));
				return false;
			}
		},
		onUpdateAgent,
		onDeleteAgent,
		onReadAgentConfig,
		onWriteAgentConfig,
		createDescriptionGenerating:
			generatingDescriptionTarget === "create" && generateDescription.isLoading,
		editDescriptionGenerating:
			generatingDescriptionTarget === "edit" && generateDescription.isLoading,
		onGenerateCreateDescription: async (seed) => {
			if (!sourceWorkspaceId || !sourceWorkspaceName) {
				setError("Add a workspace before generating agent configuration.");
				return null;
			}
			setGeneratingDescriptionTarget("create");
			setError(null);
			try {
				return await generateDescription.mutate({ target: "create", seed });
			} catch (e) {
				setError(toErrorMessage(e, "Unable to generate agent configuration."));
				return null;
			} finally {
				setGeneratingDescriptionTarget(null);
			}
		},
		onGenerateEditDescription: async (seed) => {
			if (!sourceWorkspaceId || !sourceWorkspaceName) {
				setError("Add a workspace before generating agent configuration.");
				return null;
			}
			setGeneratingDescriptionTarget("edit");
			setError(null);
			try {
				return await generateDescription.mutate({ target: "edit", seed });
			} catch (e) {
				setError(toErrorMessage(e, "Unable to generate agent configuration."));
				return null;
			} finally {
				setGeneratingDescriptionTarget(null);
			}
		},
		modelOptions,
		modelOptionsLoading,
		modelOptionsError,
	};
};
