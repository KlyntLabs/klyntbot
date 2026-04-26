// @vitest-environment jsdom
import { render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { QueryProvider } from "@/lib/query";

type Handler = (payload: unknown) => void;

// Capture every listener so we can fire events from the test.
const subs = new Map<string, Handler>();
const fakeListen = vi.fn(async (event: string, handler: Handler) => {
	subs.set(event, handler);
	return () => subs.delete(event);
});
const fakeIpc = vi.fn();

vi.mock("@/utils/tauri-bridge", () => ({
	ipc: (...args: unknown[]) => fakeIpc(...args),
	isTauri: () => true,
	getCurrentWindow: () => ({
		label: "tray",
		hide: vi.fn(),
		show: vi.fn(),
		setFocus: vi.fn(),
		setSize: vi.fn(() => Promise.resolve()),
	}),
	getWindowByLabel: () => ({
		label: "main",
		hide: vi.fn(),
		show: vi.fn(),
		setFocus: vi.fn(),
	}),
	emit: vi.fn(),
	listen: (...args: Parameters<typeof fakeListen>) => fakeListen(...args),
	currentWindowLabel: () => "tray",
}));

import { Tray } from "../components/Tray";

afterEach(() => {
	fakeIpc.mockReset();
	fakeListen.mockClear();
	subs.clear();
});

describe("Tray real-time", () => {
	it("refetches today_tasks when entity:updated{kind:'task'} fires", async () => {
		fakeIpc.mockImplementation(async (cmd: string) => {
			if (cmd === "today_tasks") return [];
			if (cmd === "productivity_calendar_events") return [];
			if (cmd === "focus_session_status")
				return { active: false, sync: null, session: null };
			if (cmd === "productivity_sessions") return [];
			if (cmd === "flashcard_total_due") return 0;
			return null;
		});

		render(
			<QueryProvider>
				<Tray />
			</QueryProvider>,
		);

		// Wait for initial fetch.
		await waitFor(() =>
			expect(fakeIpc).toHaveBeenCalledWith("today_tasks", undefined),
		);
		const initialCallCount = fakeIpc.mock.calls.filter(
			([cmd]) => cmd === "today_tasks",
		).length;

		// Fire a fake event from "another window".
		const fire = subs.get("entity:updated");
		expect(fire).toBeDefined();
		fire?.({ entityKind: "task", id: "t1" });

		// today_tasks should refetch.
		await waitFor(() => {
			const after = fakeIpc.mock.calls.filter(
				([cmd]) => cmd === "today_tasks",
			).length;
			expect(after).toBeGreaterThan(initialCallCount);
		});
	});
});
