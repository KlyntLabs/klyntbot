import { useCallback, useState } from "react";
import { ipc } from "@/utils/tauri-bridge";

// Minimal mutation hook — wraps ipc with `.mutate(args)` + loading flag.
// Mirrors the .bak's `@shared/hooks/useMutation` shape so ported callsites work.
export function useTrayMutation<TResult, TArgs extends Record<string, unknown>>(
	cmd: string,
) {
	const [loading, setLoading] = useState(false);

	const mutate = useCallback(
		async (args: TArgs): Promise<TResult | undefined> => {
			setLoading(true);
			try {
				const result = await ipc<TResult>(cmd, args);
				return result;
			} catch (err) {
				console.error(`[useTrayMutation] ${cmd} failed:`, err);
				return undefined;
			} finally {
				setLoading(false);
			}
		},
		[cmd],
	);

	return { mutate, loading };
}
