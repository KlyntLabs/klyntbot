import { QueryClient } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";
import { qk } from "../queryKeys";
import { startTauriEventBridge } from "../tauriEventBridge";

function fakeListenFactory() {
	const subs = new Map<string, (payload: unknown) => void>();
	const listen = vi.fn(async <T = unknown>(event: string, handler: (payload: T) => void) => {
		subs.set(event, handler as (payload: unknown) => void);
		return () => subs.delete(event);
	});
	const fire = (event: string, payload: unknown) =>
		subs.get(event)?.(payload);
	return { listen, fire, subs };
}

describe("tauriEventBridge", () => {
	it("entity:updated{entityKind:'task'} invalidates tasks.all", async () => {
		const client = new QueryClient();
		const spy = vi.spyOn(client, "invalidateQueries");
		const { listen, fire } = fakeListenFactory();

		const stop = await startTauriEventBridge(client, listen as any);
		fire("entity:updated", { entityKind: "task", id: "t1" });

		expect(spy).toHaveBeenCalledWith({ queryKey: qk.tasks.all() });
		stop();
	});

	it("focus:phase_changed invalidates focus.status", async () => {
		const client = new QueryClient();
		const spy = vi.spyOn(client, "invalidateQueries");
		const { listen, fire } = fakeListenFactory();

		const stop = await startTauriEventBridge(client, listen as any);
		fire("focus:phase_changed", { phase: "break" });

		expect(spy).toHaveBeenCalledWith({ queryKey: qk.focus.status() });
		stop();
	});

	it("entity:updated with unknown kind invalidates nothing", async () => {
		const client = new QueryClient();
		const spy = vi.spyOn(client, "invalidateQueries");
		const { listen, fire } = fakeListenFactory();

		const stop = await startTauriEventBridge(client, listen as any);
		fire("entity:updated", { entityKind: "unknownKind", id: "x" });

		expect(spy).not.toHaveBeenCalled();
		stop();
	});

	it("returns a cleanup that unsubscribes all events", async () => {
		const client = new QueryClient();
		const { listen, subs, fire } = fakeListenFactory();
		const spy = vi.spyOn(client, "invalidateQueries");

		const stop = await startTauriEventBridge(client, listen as any);
		expect(subs.size).toBeGreaterThan(0);
		stop();
		expect(subs.size).toBe(0);

		fire("entity:updated", { entityKind: "task", id: "t1" });
		expect(spy).not.toHaveBeenCalled();
	});
});
