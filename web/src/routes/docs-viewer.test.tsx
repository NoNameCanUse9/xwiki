import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Toaster } from "sonner";
import DocsViewerPage from "./docs-viewer";
import * as docsApi from "@/lib/api/docs";
import * as changesetsApi from "@/lib/api/changesets";
import * as historyApi from "@/lib/api/history";

vi.mock("@/lib/api/docs", () => ({
	getTree: vi.fn(),
	getPage: vi.fn(),
	getHome: vi.fn(),
}));

vi.mock("@/lib/api/changesets", () => ({
	getRevision: vi.fn(),
	submitChangeset: vi.fn(),
}));

vi.mock("@/lib/api/history", () => ({
	fileHistory: vi.fn(),
}));

vi.mock("@/lib/api/search", () => ({
	backlinks: vi.fn(),
	searchProject: vi.fn(),
}));

// jsdom 下真实 mermaid 渲染不稳定——mock 成同步空操作。
vi.mock("mermaid", async (importOriginal) => {
	const actual = await importOriginal<typeof import("mermaid")>();
	return {
		...actual,
		default: {
			...actual.default,
			initialize: vi.fn(),
			render: vi.fn().mockResolvedValue({ svg: "<svg>mmd</svg>" }),
		},
	};
});

function renderPage(path = "/projects/prj_1/docs") {
	const qc = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	return render(
		<QueryClientProvider client={qc}>
			<MemoryRouter initialEntries={[path]}>
				<Routes>
					<Route path="/projects/:id/docs/*" element={<DocsViewerPage />} />
				</Routes>
			</MemoryRouter>
			<Toaster />
		</QueryClientProvider>,
	);
}

describe("DocsViewerPage", () => {
	beforeEach(() => {
		vi.clearAllMocks();
		// _sidebar.md 查询默认失败（多数测试不关心侧栏菜单）
		vi.mocked(docsApi.getPage).mockRejectedValue(new Error("no sidebar"));
	});

	it("renders a github-style file listing at the docs root (no README)", async () => {
		vi.mocked(docsApi.getTree).mockResolvedValue({
			path: "",
			tree: [
				{ name: "docs", type: "tree", path: "docs" },
				{ name: "README.md", type: "blob", path: "README.md" },
			],
		});
		renderPage();
		await new Promise((r) => setTimeout(r, 400));
		expect(screen.queryAllByText("docs").length).toBeGreaterThanOrEqual(1);
		expect(screen.queryAllByText("README.md").length).toBeGreaterThanOrEqual(1);
		expect(docsApi.getHome).not.toHaveBeenCalled();
	});

	it("opens a directory as a github-style file listing", async () => {
		vi.mocked(docsApi.getHome).mockResolvedValue({
			path: "README.md",
			format: "html",
			content: "<p>x</p>",
		});
		vi.mocked(docsApi.getTree).mockImplementation(
			async (_id: string, path?: string) => {
				if (path === "docs") {
					return {
						path: "docs",
						tree: [
							{ name: "guide.md", type: "blob", path: "docs/guide.md" },
							{ name: "sub", type: "tree", path: "docs/sub" },
						],
					};
				}
				return {
					path: "",
					tree: [
						{ name: "docs", type: "tree", path: "docs" },
						{ name: "README.md", type: "blob", path: "README.md" },
					],
				};
			},
		);
		const user = userEvent.setup();
		renderPage();
		await user.click(await screen.findByRole("button", { name: "docs" }));
		await new Promise((r) => setTimeout(r, 400));
		// 目录列表页显示文件与子目录（GitHub 风格）
		expect(screen.queryByText("guide.md")).not.toBeNull();
		expect(screen.queryByText("sub")).not.toBeNull();
		expect(docsApi.getTree).toHaveBeenCalledWith("prj_1", "docs");
	});

	it("opens a file and shows breadcrumbs", async () => {
		vi.mocked(docsApi.getHome).mockResolvedValue({
			path: "README.md",
			format: "html",
			content: "<p>x</p>",
		});
		vi.mocked(docsApi.getTree).mockResolvedValue({
			path: "",
			tree: [{ name: "guide.md", type: "blob", path: "guide.md" }],
		});
		vi.mocked(docsApi.getPage).mockResolvedValue({
			path: "guide.md",
			format: "html",
			content: "<h1>Guide</h1>",
		});
		const user = userEvent.setup();
		renderPage();
		await user.click(await screen.findByRole("button", { name: "guide.md" }));
		await new Promise((r) => setTimeout(r, 400));
		expect(screen.queryAllByText("Guide").length).toBeGreaterThanOrEqual(1);
		expect(docsApi.getPage).toHaveBeenCalledWith(
			"prj_1",
			"guide.md",
			"html",
			undefined,
		);
		// Breadcrumb shows the file segment.
		expect(screen.getAllByText("guide.md").length).toBeGreaterThan(0);
	});

	it("shows an error state when the page is missing", async () => {
		vi.mocked(docsApi.getPage).mockRejectedValue(new Error("missing"));
		vi.mocked(docsApi.getTree).mockResolvedValue({ path: "", tree: [] });
		renderPage("/projects/prj_1/docs/missing.md");
		await new Promise((r) => setTimeout(r, 400));
		expect(screen.queryByText("文档不存在")).not.toBeNull();
	});

	it("edits a file and saves via changeset", async () => {
		vi.mocked(docsApi.getPage).mockResolvedValue({
			path: "guide.md",
			format: "html",
			content: "<h1>Guide</h1>",
		});
		vi.mocked(docsApi.getTree).mockResolvedValue({
			path: "",
			tree: [{ name: "guide.md", type: "blob", path: "guide.md" }],
		});
		vi.mocked(changesetsApi.getRevision).mockResolvedValue({
			revision: "rev1",
		});
		vi.mocked(changesetsApi.submitChangeset).mockResolvedValue({
			commit: "c1",
			revision: "c1",
		});
		// Raw content fetch for the editor.
		vi.mocked(docsApi.getPage).mockResolvedValueOnce({
			path: "guide.md",
			format: "html",
			content: "<h1>Guide</h1>",
		});
		vi.mocked(docsApi.getPage).mockResolvedValueOnce({
			path: "guide.md",
			format: "raw",
			content: "# Guide\n",
		});
		const user = userEvent.setup();
		renderPage("/projects/prj_1/docs/guide.md");
		await user.click(await screen.findByRole("button", { name: /编辑/ }));
		const editor = screen
			.getAllByRole("textbox")
			.find((el) => el.tagName !== "INPUT");
		expect(editor).toBeTruthy();
		await user.click(editor as HTMLElement);
		await user.keyboard("{Control>}a{/Control}");
		await user.keyboard("# Updated{Enter}");
		await user.click(screen.getByRole("button", { name: "保存" }));
		await vi.waitFor(() =>
			expect(changesetsApi.submitChangeset).toHaveBeenCalledWith("prj_1", {
				base_revision: "rev1",
				message: "",
				changes: [
					{
						op: "update",
						path: "guide.md",
						content: expect.stringContaining("# Updated"),
					},
				],
			}),
		);
	});

	it("shows the file history panel", async () => {
		vi.mocked(docsApi.getPage).mockResolvedValue({
			path: "guide.md",
			format: "html",
			content: "<h1>Guide</h1>",
		});
		vi.mocked(docsApi.getTree).mockResolvedValue({
			path: "",
			tree: [{ name: "guide.md", type: "blob", path: "guide.md" }],
		});
		vi.mocked(historyApi.fileHistory).mockResolvedValue({
			path: "guide.md",
			commits: [
				{
					sha: "a".repeat(40),
					message: "first",
					author: "x",
					date: "2026-08-02T00:00:00Z",
				},
			],
		});
		const user = userEvent.setup();
		renderPage("/projects/prj_1/docs/guide.md");
		await user.click(await screen.findByRole("button", { name: /历史/ }));
		await new Promise((r) => setTimeout(r, 400));
		expect(screen.queryAllByText("first").length).toBeGreaterThanOrEqual(1);
		expect(historyApi.fileHistory).toHaveBeenCalledWith("prj_1", "guide.md");
	});

	it("shows a conflict toast on 409", async () => {
		vi.mocked(docsApi.getPage).mockResolvedValue({
			path: "guide.md",
			format: "html",
			content: "<h1>Guide</h1>",
		});
		vi.mocked(docsApi.getTree).mockResolvedValue({
			path: "",
			tree: [{ name: "guide.md", type: "blob", path: "guide.md" }],
		});
		vi.mocked(changesetsApi.getRevision).mockResolvedValue({
			revision: "rev1",
		});
		vi.mocked(changesetsApi.submitChangeset).mockRejectedValue(
			new Error("stale"),
		);
		// Simulate ApiError 409 via object with status.
		vi.mocked(changesetsApi.submitChangeset).mockRejectedValueOnce(
			Object.assign(new Error("stale"), {
				status: 409,
				code: "revision_conflict",
			}),
		);
		vi.mocked(docsApi.getPage).mockResolvedValueOnce({
			path: "guide.md",
			format: "html",
			content: "<h1>Guide</h1>",
		});
		vi.mocked(docsApi.getPage).mockResolvedValueOnce({
			path: "guide.md",
			format: "raw",
			content: "# Guide\n",
		});
		const user = userEvent.setup();
		renderPage("/projects/prj_1/docs/guide.md");
		await user.click(await screen.findByRole("button", { name: /编辑/ }));
		await screen.findAllByRole("textbox");
		await user.click(screen.getByRole("button", { name: "保存" }));
		await new Promise((r) => setTimeout(r, 400));
		expect(screen.queryByText("文档已被他人修改，请刷新后重试")).not.toBeNull();
	});
});

describe("DocsViewerPage search", () => {
	it("searches and navigates to a result", async () => {
		const mod = await import("@/lib/api/search");
		vi.spyOn(mod, "searchProject").mockResolvedValue({
			query: "pineapple",
			results: [{ path: "docs/keyword.md", snippet: "walrus pineapple" }],
		});
		vi.mocked(docsApi.getHome).mockResolvedValue({
			path: "README.md",
			format: "html",
			content: "<p>x</p>",
		});
		vi.mocked(docsApi.getTree).mockResolvedValue({ path: "", tree: [] });
		vi.mocked(docsApi.getPage).mockResolvedValue({
			path: "docs/keyword.md",
			format: "html",
			content: "<h1>K</h1>",
		});
		const user = userEvent.setup();
		renderPage();
		await user.type(screen.getByLabelText("搜索文档"), "pineapple");
		await user.click(screen.getByRole("button", { name: "搜索" }));
		await new Promise((r) => setTimeout(r, 400));
		expect(screen.queryByText("docs/keyword.md")).not.toBeNull();
		await user.click(screen.getByRole("button", { name: /docs\/keyword.md/ }));
		await new Promise((r) => setTimeout(r, 400));
		expect(screen.queryAllByText("K").length).toBeGreaterThanOrEqual(1);
	});
});

describe("DocsViewerPage sidebar", () => {
	it("renders _sidebar.md menu items and hides the file from the tree", async () => {
		vi.mocked(docsApi.getHome).mockResolvedValue({
			path: "README.md",
			format: "html",
			content: "<p>x</p>",
		});
		vi.mocked(docsApi.getPage).mockImplementation(
			async (_id: string, path?: string) => {
				if (path === "_sidebar.md") {
					return {
						path: "_sidebar.md",
						format: "raw",
						content: "- [指南](docs/guide.md)\n- [首页](README.md)\n",
					};
				}
				return { path: "guide.md", format: "html", content: "<h1>Guide</h1>" };
			},
		);
		vi.mocked(docsApi.getTree).mockResolvedValue({
			path: "",
			tree: [
				{ name: "_sidebar.md", type: "blob", path: "_sidebar.md" },
				{ name: "guide.md", type: "blob", path: "guide.md" },
			],
		});
		renderPage("/projects/prj_1/docs/guide.md");
		await new Promise((r) => setTimeout(r, 400));
		expect(screen.queryByText("指南")).not.toBeNull();
		expect(screen.getByText("首页")).toBeInTheDocument();
		// _sidebar.md 不显示在树中
		expect(screen.queryByText("_sidebar.md")).not.toBeInTheDocument();
	});
});
