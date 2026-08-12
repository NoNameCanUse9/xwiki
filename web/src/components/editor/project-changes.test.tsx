import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ProjectChanges } from "./project-changes";
import { parseDocumentPatch } from "./project-changes-parser";
import * as historyApi from "@/lib/api/history";

vi.mock("@/lib/api/history", () => ({
	listCommits: vi.fn(),
	getCommitDiff: vi.fn(),
}));

function wrap(ui: React.ReactNode) {
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	return render(
		<QueryClientProvider client={client}>{ui}</QueryClientProvider>,
	);
}

describe("parseDocumentPatch", () => {
	it("extracts document paths, headings, line numbers and changed lines", () => {
		const files = parseDocumentPatch(`diff --git a/docs/auth.md b/docs/auth.md
index 111..222 100644
--- a/docs/auth.md
+++ b/docs/auth.md
@@ -8,4 +8,5 @@
 ## Token 刷新
 旧说明
-过期后重新登录
+过期前自动刷新
+失败后重新登录
 `);
		expect(files).toHaveLength(1);
		expect(files[0].path).toBe("docs/auth.md");
		expect(files[0].hunks[0].heading).toBe("Token 刷新");
		expect(files[0].hunks[0].newStart).toBe(8);
		expect(files[0].hunks[0].lines).toEqual(
			expect.arrayContaining([
				expect.objectContaining({ kind: "add", newLine: 10, content: "过期前自动刷新" }),
				expect.objectContaining({ kind: "delete", oldLine: 10, content: "过期后重新登录" }),
			]),
		);
	});
});

describe("ProjectChanges", () => {
	beforeEach(() => vi.clearAllMocks());

	it("shows detailed changes below a commit and opens its document", async () => {
		vi.mocked(historyApi.listCommits).mockResolvedValue({
			commits: [{
				sha: "a".repeat(40),
				message: "完善 Token 刷新说明",
				author: "agent",
				date: "2026-08-12T11:41:58+08:00",
			}],
		});
		vi.mocked(historyApi.getCommitDiff).mockResolvedValue({
			sha: "a".repeat(40),
			format: "patch",
			stats: [{ path: "docs/auth.md", added: 1, deleted: 0 }],
			patch: `diff --git a/docs/auth.md b/docs/auth.md
--- a/docs/auth.md
+++ b/docs/auth.md
@@ -20,2 +20,3 @@
 ## Token 刷新
+失败后重新登录`,
		});
		const onOpen = vi.fn();
		wrap(<ProjectChanges projectId="p1" onOpen={onOpen} />);

		await userEvent.click(await screen.findByText("完善 Token 刷新说明"));
		expect(historyApi.listCommits).toHaveBeenCalledWith("p1", 20, 0);
		expect(await screen.findByText("docs/auth.md:L20")).toBeInTheDocument();
		expect(screen.getByText("§ Token 刷新")).toBeInTheDocument();
		expect(screen.getByText("失败后重新登录")).toBeInTheDocument();
		await userEvent.click(screen.getByRole("button", { name: /docs\/auth\.md/ }));
		expect(onOpen).toHaveBeenCalledWith("docs/auth.md");
		expect(historyApi.getCommitDiff).toHaveBeenCalledWith("p1", "a".repeat(40), "patch");
	});
});
