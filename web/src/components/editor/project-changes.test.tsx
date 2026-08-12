import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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

	it("shows scroll arrows when the timeline overflows and scrolls right", async () => {
		vi.mocked(historyApi.listCommits).mockResolvedValue({
			commits: [{
				sha: "a".repeat(40),
				message: "初始化项目文档",
				author: "agent",
				date: "2026-08-12T07:41:58+08:00",
			}],
		});
		wrap(<ProjectChanges projectId="p1" />);

		const region = await screen.findByRole("region", { name: "项目变更时间线" });
		expect(screen.queryByRole("button", { name: "向左滚动时间线" })).not.toBeInTheDocument();
		const scrollArea = region;
		expect(scrollArea).toHaveAttribute("data-timeline-scroll");
		Object.defineProperties(scrollArea!, {
			clientWidth: { configurable: true, value: 300 },
			scrollWidth: { configurable: true, value: 720 },
			scrollLeft: { configurable: true, writable: true, value: 0 },
		});
		const scrollBy = vi.fn();
		Object.defineProperty(scrollArea, "scrollBy", { configurable: true, value: scrollBy });
		fireEvent(window, new Event("resize"));

		expect(await screen.findByRole("button", { name: "向左滚动时间线" })).toBeDisabled();
		const rightArrow = screen.getByRole("button", { name: "向右滚动时间线" });
		expect(rightArrow).not.toBeDisabled();
		await userEvent.click(rightArrow);
		expect(scrollBy).toHaveBeenCalledWith(expect.objectContaining({ behavior: "smooth" }));

		Object.defineProperty(scrollArea, "scrollLeft", { configurable: true, writable: true, value: 420 });
		fireEvent.scroll(scrollArea);
		await waitFor(() => {
			expect(screen.getByRole("button", { name: "向左滚动时间线" })).not.toBeDisabled();
			expect(screen.getByRole("button", { name: "向右滚动时间线" })).toBeDisabled();
		});
	});

	it("renders the latest five commits with visible titles and hover details", async () => {
		vi.mocked(historyApi.listCommits).mockResolvedValue({
			commits: [
				{
					sha: "a".repeat(40),
					message: "完善 Token 刷新说明",
					author: "agent",
					date: "2026-08-12T11:41:58+08:00",
				},
				{
					sha: "b".repeat(40),
					message: "补充鉴权示例",
					author: "agent",
					date: "2026-08-12T10:41:58+08:00",
				},
				{
					sha: "c".repeat(40),
					message: "调整文档导航",
					author: "agent",
					date: "2026-08-12T09:41:58+08:00",
				},
				{
					sha: "d".repeat(40),
					message: "新增安装说明",
					author: "agent",
					date: "2026-08-12T08:41:58+08:00",
				},
				{
					sha: "e".repeat(40),
					message: "初始化项目文档并补充完整的历史说明与版本回退指南",
					author: "agent",
					date: "2026-08-12T07:41:58+08:00",
				},
			],
		});
		wrap(<ProjectChanges projectId="p1" />);

		const nodes = await screen.findAllByRole("img", { name: /^提交：/ });
		expect(historyApi.listCommits).toHaveBeenCalledWith("p1", 5, 0);
		expect(screen.queryByText("changes · roadmap")).not.toBeInTheDocument();
		expect(screen.queryByText("最近 5 次")).not.toBeInTheDocument();
		expect(screen.getByText("初始化项目文档并补充完整的历史说明与版本回退指南")).toBeInTheDocument();
		expect(screen.getByRole("region", { name: "项目变更记录" }).className).toContain("mt-auto");
		expect(screen.getByRole("region", { name: "项目变更记录" }).className).toContain("pt-24");
		const timeline = screen.getByRole("region", { name: "项目变更时间线" });
		expect(timeline.className).toContain("overflow-x-auto");
		expect(nodes.map((node) => node.getAttribute("aria-label"))).toEqual([
			"提交：初始化项目文档并补充完整的历史说明与版本回退指南",
			"提交：新增安装说明",
			"提交：调整文档导航",
			"提交：补充鉴权示例",
			"提交：完善 Token 刷新说明",
		]);
		expect(nodes.map((node) => node.getAttribute("data-side"))).toEqual([
			"top",
			"bottom",
			"top",
			"bottom",
			"top",
		]);
		const markers = timeline.querySelectorAll('[data-track-marker="true"]');
		const ticks = timeline.querySelectorAll('[data-track-tick="true"]');
		expect(markers).toHaveLength(5);
		expect(ticks).toHaveLength(5);
		expect([...markers].every((marker) => marker.className.includes("top-1/2"))).toBe(true);
		expect([...ticks].every((tick) => tick.className.includes("left-1/2"))).toBe(true);
		expect([...markers].every((marker) => marker.getAttribute("data-track-anchor"))).toBe(true);
		expect([...ticks].every((tick) => tick.getAttribute("data-track-anchor"))).toBe(true);
		expect(screen.getByText("初始化项目文档并补充完整的历史说明与版本回退指南")).toBeInTheDocument();
		await userEvent.hover(nodes[0]);
		expect(await screen.findByRole("tooltip")).toHaveTextContent("初始化项目文档并补充完整的历史说明与版本回退指南");
		expect(screen.getByRole("tooltip")).toHaveTextContent("agent");
		await userEvent.unhover(nodes[0]);

		expect(historyApi.getCommitDiff).not.toHaveBeenCalled();
	});
});
