import {
	type QueryKey,
	useMutation,
	useQueryClient,
} from "@tanstack/react-query";
import { ipc } from "@/utils/tauri-bridge";
import { entityKindForCommand } from "./entityKindMap";

export interface OptimisticConfig<TVars, TPrev> {
	queryKey: QueryKey;
	update: (vars: TVars, prev: TPrev) => TPrev;
}

export interface TauriMutationOptions<TData, TVars> {
	command: string;
	/**
	 * Override the auto-derived invalidation. Pass an empty array to skip.
	 * Default: invalidates the entity-domain bucket inferred from the command.
	 */
	invalidates?: QueryKey[];
	/** Opt-in optimistic patch. Rolls back on error. */
	// biome-ignore lint/suspicious/noExplicitAny: TPrev is opaque to the hook
	optimistic?: OptimisticConfig<TVars, any>;
	onSuccess?: (data: TData, vars: TVars) => void;
	onError?: (error: unknown, vars: TVars) => void;
}

export function useTauriMutation<TData = unknown, TVars = void>(
	opts: TauriMutationOptions<TData, TVars>,
) {
	const client = useQueryClient();

	const mutation = useMutation<
		TData,
		unknown,
		TVars,
		{ rollback?: () => void }
	>({
		mutationFn: (vars) =>
			ipc<TData>(opts.command, vars as Record<string, unknown> | undefined),

		onMutate: async (vars) => {
			if (!opts.optimistic) return {};
			const { queryKey, update } = opts.optimistic;
			await client.cancelQueries({ queryKey });
			const prev = client.getQueryData(queryKey);
			client.setQueryData(queryKey, (old: unknown) => update(vars, old));
			return { rollback: () => client.setQueryData(queryKey, prev) };
		},

		onError: (err, vars, ctx) => {
			ctx?.rollback?.();
			opts.onError?.(err, vars);
		},

		onSuccess: (data, vars) => {
			opts.onSuccess?.(data, vars);
		},

		onSettled: () => {
			const overrides = opts.invalidates;
			if (overrides) {
				for (const key of overrides) client.invalidateQueries({ queryKey: key });
				return;
			}
			const kind = entityKindForCommand(opts.command);
			if (kind) {
				// Broad-prefix invalidation: queries starting with [kindRoot]
				// match. e.g. ["tasks"] invalidates ["tasks","today"], etc.
				const root = kind === "task" ? "tasks" : kind;
				client.invalidateQueries({ queryKey: [root] });
			}
		},
	});

	return {
		mutate: mutation.mutateAsync,
		isLoading: mutation.isPending,
		error: mutation.error,
	};
}
