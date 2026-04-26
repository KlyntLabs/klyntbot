import { QueryClient } from "@tanstack/react-query";

// One QueryClient per webview. Exposed as a factory (not a singleton) so each
// Tauri window's React tree gets its own cache; events broadcast across
// windows reach all clients via tauriEventBridge.
export function createQueryClient(): QueryClient {
	return new QueryClient({
		defaultOptions: {
			queries: {
				// Tauri webviews trigger "focus" events constantly during dev
				// reload + multi-window setups; the browser-default refetch
				// would stampede.
				refetchOnWindowFocus: false,
				// 30s mirrors the .bak's default. Push events drive freshness;
				// staleTime is just the "no events seen, we'd better double-check"
				// safety net.
				staleTime: 30_000,
				// Tauri command failures are usually deterministic (handler not
				// registered, type mismatch). Retrying 3× wastes time.
				retry: 1,
			},
			mutations: {
				retry: 0,
			},
		},
	});
}
