import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { extractToc, HistoryPanel, TocPanel, VersionPanel } from "./version-toc";
import * as historyApi from "@/lib/api/history";

vi.mock("@/lib/api/history", () => ({
	fileHistory: vi.fn(),
	getCommitDiff: vi.fn(),
}));

function wrap(ui: React.ReactNode) {
	const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
	return render(
		<QueryClientProvider client={qc}>
			<MemoryRouter>{ui}</MemoryRouter>
		</QueryClientProvider>,
	);
}

describe("extractToc", () => {
	it("extracts h1-h3 with ids", () => {
		const root = document.createElement("div");
		root.innerHTML = "<h1>Title</h1><h2>Sub</h2><h4>Skip</h4><p>x</p>";
		const entries = extractToc(root);
		expect(entries.map((e) => e.text)).toEqual(["Title", "Sub"]);
		expect(entries[0].level).toBe(1);
		expect(entries[1].level).toBe(2);
		expect(root.querySelector("h1")?.id).toBeTruthy();
	});

	it("excludes rendered yaml front matter headings", () => {
		const root = document.createElement("div");
		root.innerHTML = `
			<hr>
			<h2>title: 资讯管理\nmodule: 资讯管理\nversion: v1.0\nsummary: 平台资讯的创建、查询、详情、修改</h2>
			<h1>资讯管理</h1>
			<h2>创建资讯</h2>
		`;

		expect(extractToc(root).map((entry) => entry.text)).toEqual([
			"资讯管理",
			"创建资讯",
		]);
	});

	it("keeps ordinary headings that contain a colon", () => {
		const root = document.createElement("div");
		root.innerHTML = "<hr><h2>API: Overview</h2>";

		expect(extractToc(root).map((entry) => entry.text)).toEqual([
			"API: Overview",
		]);
	});
});

describe("TocPanel", () => {
	it("renders entries and scrolls on click", async () => {
		const scrollIntoView = vi.fn();
		Element.prototype.scrollIntoView = scrollIntoView;
		wrap(
			<>
				<h1 id="toc-0">Title</h1>
				<h2 id="toc-1">Sub</h2>
				<TocPanel
					entries={[
						{ id: "toc-0", index: 0, text: "Title", level: 1 },
						{ id: "toc-1", index: 1, text: "Sub", level: 2 },
					]}
				/>
			</>,
		);
		expect(screen.queryAllByText("Title").length).toBeGreaterThanOrEqual(1);
		await userEvent.click(screen.queryAllByText("Sub")[1]);
		expect(scrollIntoView).toHaveBeenCalled();
	});
});

describe("VersionPanel", () => {
	beforeEach(() => vi.clearAllMocks());

	it("lists versions and calls onSelect", async () => {
		vi.mocked(historyApi.fileHistory).mockResolvedValue({
			path: "docs/a.md",
			commits: [
				{
					sha: "a".repeat(40),
					message: "first",
					author: "x",
					date: "2026-08-02T00:00:00Z",
				},
				{
					sha: "b".repeat(40),
					message: "second",
					author: "x",
					date: "2026-08-02T01:00:00Z",
				},
			],
		});
		const onSelect = vi.fn();
		wrap(
			<VersionPanel
				projectId="prj_1"
				filePath="docs/a.md"
				currentVersion={null}
				onSelect={onSelect}
			/>,
		);
		expect(
			await screen.findByRole("button", { name: /aaaaaaa/ }),
		).toBeInTheDocument();
		await userEvent.click(screen.getByRole("button", { name: /aaaaaaa/ }));
		expect(onSelect).toHaveBeenCalledWith("a".repeat(40));
		await userEvent.click(screen.getByText("最新版本"));
		expect(onSelect).toHaveBeenCalledWith(null);
	});
});

describe("HistoryPanel", () => {
	beforeEach(() => vi.clearAllMocks());

	it("shows author, datetime and full sha per commit", async () => {
		vi.mocked(historyApi.fileHistory).mockResolvedValue({
			path: "docs/a.md",
			commits: [
				{
					sha: "8b6ea694750e4ca4e398db4e91b5e21e6c42268b",
					message: "补充颜色说明",
					author: "admin",
					date: "2026-08-12T11:41:58+08:00",
				},
				{
					sha: "85680604050f6f3d166c438d086ca3e4afdfa6cb",
					message: "add cabinet note doc",
					author: "alice",
					date: "2026-08-11T09:00:00+08:00",
				},
			],
		});
		wrap(
			<HistoryPanel
				projectId="p1"
				filePath="docs/a.md"
				currentVersion={null}
				onSelect={vi.fn()}
				onClose={vi.fn()}
			/>,
		);
		expect(await screen.findByText(/admin/)).toBeInTheDocument();
		expect(screen.getByText(/alice/)).toBeInTheDocument();
		expect(
			screen.getByText("8b6ea694750e4ca4e398db4e91b5e21e6c42268b"),
		).toBeInTheDocument();
	});

	it("loads and shows per-file diff stats on expand", async () => {
		vi.mocked(historyApi.fileHistory).mockResolvedValue({
			path: "docs/a.md",
			commits: [
				{
					sha: "8b6ea694750e4ca4e398db4e91b5e21e6c42268b",
					message: "补充颜色说明",
					author: "admin",
					date: "2026-08-12T11:41:58+08:00",
				},
			],
		});
		vi.mocked(historyApi.getCommitDiff).mockResolvedValue({
			sha: "8b6ea694750e4ca4e398db4e91b5e21e6c42268b",
			format: "numstat",
			stats: [{ path: "docs/a.md", added: 3, deleted: 1 }],
		});
		wrap(
			<HistoryPanel
				projectId="p1"
				filePath="docs/a.md"
				currentVersion={null}
				onSelect={vi.fn()}
				onClose={vi.fn()}
			/>,
		);
		await userEvent.click(await screen.findByLabelText("展开详情"));
		expect(await screen.findByText(/\+3 -1/)).toBeInTheDocument();
		expect(screen.getAllByText(/docs\/a\.md/).length).toBeGreaterThan(0);
	});
});
