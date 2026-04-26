// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@/utils/tauri-bridge", () => ({
	ipc: vi.fn(),
}));

import { ipc } from "@/utils/tauri-bridge";
import { qk } from "../queryKeys";
import { useTauriQuery } from "../useTauriQuery";

const mockedIpc = vi.mocked(ipc);

afterEach(() => {
	mockedIpc.mockReset();
});

function wrapper(client: QueryClient) {
	return ({ children }: { children: ReactNode }) => (
		<QueryClientProvider client={client}>{children}</QueryClientProvider>
	);
}

describe("useTauriQuery", () => {
	it("calls the matching ipc command and returns its data", async () => {
		mockedIpc.mockResolvedValueOnce([{ id: "1" }]);
		const client = new QueryClient({
			defaultOptions: { queries: { retry: 0 } },
		});

		const { result } = renderHook(
			() =>
				useTauriQuery({
					queryKey: qk.tasks.today(),
					command: "today_tasks",
				}),
			{ wrapper: wrapper(client) },
		);

		await waitFor(() => expect(result.current.data).toEqual([{ id: "1" }]));
		expect(mockedIpc).toHaveBeenCalledWith("today_tasks", undefined);
	});

	it("returns the fallback while loading the first time", async () => {
		mockedIpc.mockImplementation(() => new Promise(() => {})); // never resolves
		const client = new QueryClient({
			defaultOptions: { queries: { retry: 0 } },
		});

		const { result } = renderHook(
			() =>
				useTauriQuery({
					queryKey: qk.tasks.today(),
					command: "today_tasks",
					fallback: [],
				}),
			{ wrapper: wrapper(client) },
		);

		expect(result.current.data).toEqual([]);
		expect(result.current.isFetching).toBe(true);
	});

	it("forwards args to ipc", async () => {
		mockedIpc.mockResolvedValueOnce([]);
		const client = new QueryClient({
			defaultOptions: { queries: { retry: 0 } },
		});

		renderHook(
			() =>
				useTauriQuery({
					queryKey: qk.calendar.eventsForDate("2026-04-26"),
					command: "productivity_calendar_events",
					args: { date: "2026-04-26" },
				}),
			{ wrapper: wrapper(client) },
		);

		await waitFor(() =>
			expect(mockedIpc).toHaveBeenCalledWith(
				"productivity_calendar_events",
				{ date: "2026-04-26" },
			),
		);
	});
});

describe("useTauriQuery — queryFn escape hatch", () => {
	it("uses queryFn when provided and ignores command", async () => {
		const queryFn = vi.fn().mockResolvedValue({ id: 1, name: "x" });
		const client = new QueryClient({
			defaultOptions: { queries: { retry: 0 } },
		});

		const { result } = renderHook(
			() =>
				useTauriQuery({
					queryKey: ["custom", "thing"],
					queryFn,
				}),
			{ wrapper: wrapper(client) },
		);

		await waitFor(() =>
			expect(result.current.data).toEqual({ id: 1, name: "x" }),
		);
		expect(queryFn).toHaveBeenCalledTimes(1);
		expect(mockedIpc).not.toHaveBeenCalled();
	});

	it("throws if neither command nor queryFn is provided", () => {
		expect(() => {
			useTauriQuery({ queryKey: ["empty"] } as never);
		}).toThrow("useTauriQuery: either `command` or `queryFn` must be provided");
	});
});
