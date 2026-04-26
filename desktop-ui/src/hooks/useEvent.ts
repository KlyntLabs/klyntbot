import { useEffect } from "react";
import { listen } from "@/utils/tauri-bridge";

// Subscribe to a Tauri event and call `handler` with the payload.
// `handler` is held via closure each render — caller is responsible for
// stability (or accept that listener re-mounts on every change).
export function useEvent<T = unknown>(
	event: string,
	handler: (payload: T) => void,
) {
	useEffect(() => {
		let unlisten: (() => void) | undefined;
		let cancelled = false;
		listen<T>(event, (payload) => handler(payload)).then((fn) => {
			if (cancelled) fn();
			else unlisten = fn;
		});
		return () => {
			cancelled = true;
			unlisten?.();
		};
	}, [event, handler]);
}
