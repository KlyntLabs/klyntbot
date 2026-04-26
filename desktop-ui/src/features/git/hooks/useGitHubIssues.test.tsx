// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import { createElement } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { WorkspaceInfo } from "@/types";
import { getGitHubIssues } from "@services/tauri";
import { useGitHubIssues } from "./useGitHubIssues";

vi.mock("../../../services/tauri", () => ({
	getGitHubIssues: vi.fn(),
}));

function withQuery({ children }: { children: React.ReactNode }) {
	const client = new QueryClient({
		defaultOptions: { queries: { retry: 0 } },
	});
	return createElement(QueryClientProvider, { client }, children);
}

const workspace: WorkspaceInfo = {
	id: "workspace-1",
	name: "Klynt",
	path: "/tmp/codex",
	connected: true,
	settings: { sidebarCollapsed: false },
};

describe("useGitHubIssues", () => {
	afterEach(() => {
		vi.clearAllMocks();
	});

	it("loads issues successfully", async () => {
		const getGitHubIssuesMock = vi.mocked(getGitHubIssues);
		getGitHubIssuesMock.mockResolvedValueOnce({
			total: 1,
			issues: [
				{
					number: 123,
					title: "Issue title",
					url: "https://example.com/issue/123",
					updatedAt: "2025-01-01T00:00:00Z",
				},
			],
		});

		const { result, unmount } = renderHook(
			({ active }: { active: WorkspaceInfo | null }) =>
				useGitHubIssues(active),
			{
				initialProps: { active: workspace },
				wrapper: withQuery,
			},
		);

		await waitFor(() => expect(result.current.issues).toHaveLength(1));

		expect(getGitHubIssuesMock).toHaveBeenCalledWith("workspace-1");
		expect(result.current.total).toBe(1);
		expect(result.current.error).toBeNull();

		unmount();
	});

	it("handles empty issue lists", async () => {
		const getGitHubIssuesMock = vi.mocked(getGitHubIssues);
		getGitHubIssuesMock.mockResolvedValueOnce({ total: 0, issues: [] });

		const { result, unmount } = renderHook(
			({ active }: { active: WorkspaceInfo | null }) =>
				useGitHubIssues(active),
			{
				initialProps: { active: workspace },
				wrapper: withQuery,
			},
		);

		await waitFor(() => expect(result.current.isLoading).toBe(false));

		expect(result.current.issues).toHaveLength(0);
		expect(result.current.total).toBe(0);
		expect(result.current.error).toBeNull();

		unmount();
	});

	it("surfaces fetch errors", async () => {
		const getGitHubIssuesMock = vi.mocked(getGitHubIssues);
		getGitHubIssuesMock.mockRejectedValueOnce(new Error("boom"));
		const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

		const { result, unmount } = renderHook(
			({ active }: { active: WorkspaceInfo | null }) =>
				useGitHubIssues(active),
			{
				initialProps: { active: workspace },
				wrapper: withQuery,
			},
		);

		await waitFor(() => expect(result.current.error).not.toBeNull());

		expect(result.current.issues).toHaveLength(0);
		expect(result.current.total).toBe(0);
		expect(result.current.error).toMatch(/boom/);

		errorSpy.mockRestore();
		unmount();
	});
});
