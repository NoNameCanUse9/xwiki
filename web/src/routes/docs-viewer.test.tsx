import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Toaster } from "sonner";
import DocsViewerPage from "./docs-viewer";
import * as docsApi from "@/lib/api/docs";
import * as changesetsApi from "@/lib/api/changesets";
import * as historyApi from "@/lib/api/history";
import * as locksApi from "@/lib/api/locks";
import type { EditLock } from "@/lib/api/locks";
import * as sharesApi from "@/lib/api/shares";

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

vi.mock("@/lib/api/locks", () => ({
	getLock: vi.fn(),
	acquireLock: vi.fn(),
	releaseLock: vi.fn(),
	heartbeatLock: vi.fn(),
	forceReleaseLock: vi.fn(),
	lockFromError: vi.fn(),
}));

vi.mock("@/lib/api/shares", () => ({
	createShare: vi.fn(),
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

// jsdom has no layout engine: ProseMirror calls getClientRects()/
// getBoundingClientRect()/elementFromPoint() while handling clicks and DOM
// changes. Polyfill them so the editor can be focused and typed into.
const emptyRect = () => ({
	top: 0,
	bottom: 0,
	left: 0,
	right: 0,
	width: 0,
	height: 0,
	x: 0,
	y: 0,
	toJSON: () => ({}),
});
if (typeof Range.prototype.getClientRects !== "function") {
	Object.defineProperty(Range.prototype, "getClientRects", {
		configurable: true,
		value: () => [],
	});
}
if (typeof Range.prototype.getBoundingClientRect !== "function") {
	Object.defineProperty(Range.prototype, "getBoundingClientRect", {
		configurable: true,
		value: emptyRect,
	});
}
if (typeof document.elementFromPoint !== "function") {
	document.elementFromPoint = () => null;
}

function renderPage(path = "/projects/prj_1/docs") {
	const qc = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	// Data router (not plain <MemoryRouter>): docs-viewer uses useBlocker for
	// unsaved-edit navigation guards, which requires a data router.
	const router = createMemoryRouter(
		[{ path: "/projects/:id/docs/*", element: <DocsViewerPage /> }],
		{ initialEntries: [path] },
	);
	return render(
		<QueryClientProvider client={qc}>
			<RouterProvider router={router} />
			<Toaster />
		</QueryClientProvider>,
	);
}

describe("DocsViewerPage", () => {
	beforeEach(() => {
		vi.clearAllMocks();
		localStorage.clear();
		// _sidebar.md 查询默认失败（多数测试不关心侧栏菜单）
		vi.mocked(docsApi.getPage).mockRejectedValue(new Error("no sidebar"));
		// 编辑锁默认可获取/可释放/可续期。
		const lock = {
			path: "guide.md",
			user_id: "me",
			username: "admin",
			acquired_at: new Date().toISOString(),
			expires_at: new Date(Date.now() + 300000).toISOString(),
		};
		vi.mocked(locksApi.acquireLock).mockResolvedValue({ lock });
		vi.mocked(locksApi.releaseLock).mockResolvedValue({ released: true });
		vi.mocked(locksApi.heartbeatLock).mockResolvedValue({ lock });
		vi.mocked(locksApi.getLock).mockResolvedValue({ lock: null });
		vi.mocked(locksApi.lockFromError).mockImplementation(
			(err) =>
				((err as { data?: { lock?: EditLock } })?.data?.lock ?? null) as EditLock | null,
		);
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
		vi.mocked(docsApi.getPage).mockImplementation(
			async (_id: string, path?: string, format?: string) => {
				if (path === "guide.md" && format === "raw") {
					return { path: "guide.md", format: "raw", content: "# Guide\n", revision: "loaded-rev" };
				}
				return { path: path ?? "guide.md", format: "html", content: "<h1>Guide</h1>", revision: "loaded-rev" };
			},
		);
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
		const user = userEvent.setup();
		renderPage("/projects/prj_1/docs/guide.md");
		await user.click(await screen.findByRole("button", { name: /解锁编辑/ }));
		await screen.findAllByRole("textbox");
		const editor = screen
			.getAllByRole("textbox")
			.find((el) => el.tagName !== "INPUT");
		expect(editor).toBeTruthy();
		await user.click(editor as HTMLElement);
		await user.keyboard("{Control>}a{/Control}");
		await user.keyboard("# Updated{Enter}");
		await user.click(screen.getByRole("button", { name: "上锁并提交" }));
		await user.click(screen.getByRole("button", { name: "提交并上锁" }));
		await vi.waitFor(() =>
			expect(changesetsApi.submitChangeset).toHaveBeenCalledWith("prj_1", {
				base_revision: "loaded-rev",
				message: "",
				changes: [
					{
						op: "update",
						path: "guide.md",
						content: expect.stringContaining("Updated"),
					},
				],
			}),
		);
		expect(changesetsApi.getRevision).not.toHaveBeenCalled();
		// Lock released when finishing the edit session.
		await vi.waitFor(() =>
			expect(locksApi.releaseLock).toHaveBeenCalledWith("prj_1", "guide.md"),
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
		vi.mocked(docsApi.getPage).mockImplementation(
			async (_id: string, path?: string, format?: string) => {
				if (path === "guide.md" && format === "raw") {
					return { path: "guide.md", format: "raw", content: "# Guide\n", revision: "rev1" };
				}
				return { path: path ?? "guide.md", format: "html", content: "<h1>Guide</h1>", revision: "rev1" };
			},
		);
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
		const user = userEvent.setup();
		renderPage("/projects/prj_1/docs/guide.md");
		await user.click(await screen.findByRole("button", { name: /解锁编辑/ }));
		await screen.findAllByRole("textbox");
		// Type to make the draft dirty, then lock (commits; the 409 surfaces).
		const editor = screen
			.getAllByRole("textbox")
			.find((el) => el.tagName !== "INPUT");
		await user.click(editor as HTMLElement);
		await user.keyboard("x");
		await new Promise((r) => setTimeout(r, 100));
		await user.click(screen.getByRole("button", { name: "上锁并提交" }));
		await user.click(screen.getByRole("button", { name: "提交并上锁" }));
		await new Promise((r) => setTimeout(r, 400));
		expect(screen.queryByText("文档已被他人修改，请刷新后重试")).not.toBeNull();
		expect(
			screen
				.queryAllByRole("textbox")
				.find((el) => el.tagName !== "INPUT"),
		).toBeTruthy();
		expect(locksApi.releaseLock).not.toHaveBeenCalled();
		const saved = JSON.parse(
			localStorage.getItem("agentdocs:draft:prj_1:guide.md") ?? "null",
		);
		expect(saved).toMatchObject({
			version: 1,
			project_id: "prj_1",
			path: "guide.md",
			base_revision: "rev1",
		});
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
		await user.click(screen.getByRole("button", { name: "搜索" }));
		await user.type(screen.getByLabelText("搜索文档"), "pineapple");
		await new Promise((r) => setTimeout(r, 400));
		expect(screen.queryByText("docs/keyword.md")).not.toBeNull();
		await user.click(screen.getByRole("button", { name: /docs\/keyword.md/ }));
		await new Promise((r) => setTimeout(r, 400));
		expect(screen.queryAllByText("K").length).toBeGreaterThanOrEqual(1);
	});

	it("closes the search overlay on Escape", async () => {
		const user = userEvent.setup();
		renderPage();
		await user.click(screen.getByRole("button", { name: "搜索" }));
		expect(screen.getByLabelText("搜索文档")).toBeInTheDocument();
		await user.keyboard("{Escape}");
		expect(screen.queryByLabelText("搜索文档")).not.toBeInTheDocument();
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

describe("DocsViewerPage edit sessions", () => {
	const getEditor = () =>
		screen
			.queryAllByRole("textbox")
			.find((el) => el.tagName !== "INPUT");

	function mockGuidePage() {
		vi.mocked(docsApi.getPage).mockImplementation(
			async (_id: string, path?: string, format?: string) => {
				if (path === "guide.md" && format === "raw") {
					return {
						path: "guide.md",
						format: "raw",
						content: "# Guide\n",
						revision: "rev1",
					};
				}
				return { path: "guide.md", format: "html", content: "<h1>Guide</h1>" };
			},
		);
	}

	it("requires an explicit choice for a draft based on an older revision", async () => {
		mockGuidePage();
		localStorage.setItem(
			"agentdocs:draft:prj_1:guide.md",
			JSON.stringify({
				version: 1,
				project_id: "prj_1",
				path: "guide.md",
				content: "# Local draft\n",
				base_revision: "old-rev",
				updated_at: "2026-08-12T12:00:00Z",
			}),
		);
		const user = userEvent.setup();
		renderPage("/projects/prj_1/docs/guide.md");

		await user.click(await screen.findByRole("button", { name: /解锁编辑/ }));
		expect(await screen.findByText(/本地草稿基于旧版本/)).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "恢复本地草稿" })).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "使用服务器版本" })).toBeInTheDocument();
		expect(getEditor()).toBeUndefined();
	});

	beforeEach(() => {
		localStorage.clear();
		vi.mocked(docsApi.getTree).mockResolvedValue({
			path: "",
			tree: [{ name: "guide.md", type: "blob", path: "guide.md" }],
		});
		const lock = {
			path: "guide.md",
			user_id: "me",
			username: "admin",
			acquired_at: new Date().toISOString(),
			expires_at: new Date(Date.now() + 300000).toISOString(),
		};
		vi.mocked(locksApi.acquireLock).mockResolvedValue({ lock });
		vi.mocked(locksApi.releaseLock).mockResolvedValue({ released: true });
		vi.mocked(locksApi.heartbeatLock).mockResolvedValue({ lock });
		vi.mocked(locksApi.getLock).mockResolvedValue({ lock: null });
		vi.mocked(locksApi.lockFromError).mockImplementation(
			(err) =>
				((err as { data?: { lock?: EditLock } })?.data?.lock ?? null) as EditLock | null,
		);
	});

	it("re-opening the same file for editing shows the saved content, not an empty editor", async () => {
		mockGuidePage();
		vi.mocked(changesetsApi.getRevision).mockResolvedValue({
			revision: "rev1",
		});
		vi.mocked(changesetsApi.submitChangeset).mockResolvedValue({
			commit: "c1",
			revision: "c1",
		});
		const user = userEvent.setup();
		renderPage("/projects/prj_1/docs/guide.md");

		// First session: save the untouched content.
		await user.click(await screen.findByRole("button", { name: /解锁编辑/ }));
		await screen.findAllByRole("textbox");
		expect(getEditor()?.textContent).toContain("Guide");
		await user.click(screen.getByRole("button", { name: "上锁并提交" }));
		await user.click(screen.getByRole("button", { name: "提交并上锁" }));
		await new Promise((r) => setTimeout(r, 100));

		// The raw fetch now returns the post-save content.
		vi.mocked(docsApi.getPage).mockImplementation(
			async (_id: string, path?: string, format?: string) => {
				if (path === "guide.md" && format === "raw") {
					return {
						path: "guide.md",
						format: "raw",
						content: "# Updated\n",
						revision: "c1",
					};
				}
				return { path: "guide.md", format: "html", content: "<h1>Guide</h1>" };
			},
		);

		// Second session of the same file. Regression: the raw query data is
		// cached with an unchanged reference, so the draft-fill effect used to
		// never re-run and the editor opened empty (saving would wipe the doc).
		await user.click(await screen.findByRole("button", { name: /解锁编辑/ }));
		await screen.findAllByRole("textbox");
		const editor2 = getEditor();
		await new Promise((r) => setTimeout(r, 100));
		expect(editor2?.textContent).toContain("Updated");
	});

	it("edit -> cancel -> edit the same file again keeps the content", async () => {
		mockGuidePage();
		const user = userEvent.setup();
		renderPage("/projects/prj_1/docs/guide.md");

		await user.click(await screen.findByRole("button", { name: /解锁编辑/ }));
		await screen.findAllByRole("textbox");
		expect(getEditor()?.textContent).toContain("Guide");

		await user.click(screen.getByRole("button", { name: "上锁并提交" }));
		await user.click(screen.getByRole("button", { name: "提交并上锁" }));
		await new Promise((r) => setTimeout(r, 50));
		expect(getEditor()).toBeUndefined();

		await user.click(await screen.findByRole("button", { name: /解锁编辑/ }));
		await screen.findAllByRole("textbox");
		await new Promise((r) => setTimeout(r, 100));
		expect(getEditor()?.textContent).toContain("Guide");
	});

	it("navigating to the docs root while editing must not show the editor UI in the file list", async () => {
		mockGuidePage();
		vi.mocked(docsApi.getHome).mockResolvedValue({
			path: "README.md",
			format: "html",
			content: "<p>x</p>",
		});
		const user = userEvent.setup();
		renderPage("/projects/prj_1/docs/guide.md");

		await user.click(await screen.findByRole("button", { name: /解锁编辑/ }));
		await screen.findAllByRole("textbox");
		expect(getEditor()?.textContent).toContain("Guide");

		// Navigate to the docs root via the breadcrumb while still editing.
		await user.click(screen.getByRole("link", { name: "docs" }));
		await new Promise((r) => setTimeout(r, 100));

		// The file list must not contain the editor UI.
		expect(getEditor()).toBeUndefined();
	});

	it("blocks navigation while editing with unsaved changes (cancel keeps editing)", async () => {
		mockGuidePage();
		const user = userEvent.setup();
		renderPage("/projects/prj_1/docs/guide.md");

		await user.click(await screen.findByRole("button", { name: /解锁编辑/ }));
		await screen.findAllByRole("textbox");
		const editor = getEditor() as HTMLElement;
		await user.click(editor);
		await user.keyboard("x"); // makes the draft dirty
		await new Promise((r) => setTimeout(r, 100));

		const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);
		await user.click(screen.getByRole("link", { name: "docs" }));
		await new Promise((r) => setTimeout(r, 100));
		expect(confirmSpy).toHaveBeenCalled();
		// Still on the file page, editor still visible.
		expect(getEditor()).toBeTruthy();
	});

	it("blocks navigation while editing with unsaved changes (confirm proceeds)", async () => {
		mockGuidePage();
		const user = userEvent.setup();
		renderPage("/projects/prj_1/docs/guide.md");

		await user.click(await screen.findByRole("button", { name: /解锁编辑/ }));
		await screen.findAllByRole("textbox");
		const editor = getEditor() as HTMLElement;
		await user.click(editor);
		await user.keyboard("x");
		await new Promise((r) => setTimeout(r, 100));

		const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
		await user.click(screen.getByRole("link", { name: "docs" }));
		await new Promise((r) => setTimeout(r, 100));
		expect(confirmSpy).toHaveBeenCalled();
		// Navigation proceeded: the file list is shown without the editor UI.
		expect(getEditor()).toBeUndefined();
	});

	it("hides the edit entry while viewing a historical version", async () => {
		mockGuidePage();
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

		// Select a historical version from the sidebar versions panel.
		const versionButton = await screen.findByRole("button", {
			name: /aaaaaaa · first/,
		});
		await user.click(versionButton);
		await new Promise((r) => setTimeout(r, 100));
		expect(screen.queryByText(/viewing historical version/)).not.toBeNull();

		// Editing is not offered while viewing history.
		expect(screen.queryByRole("button", { name: /解锁编辑/ })).toBeNull();
	});

	it("copies a share link for the current page", async () => {
		mockGuidePage();
		vi.mocked(sharesApi.createShare).mockResolvedValue({
			token: "t1",
			url: "/share/t1",
		});
		const user = userEvent.setup();
		renderPage("/projects/prj_1/docs/guide.md");

		await user.click(await screen.findByRole("button", { name: "分享" }));
		// The share API is called for the current page with the page path.
		await vi.waitFor(() =>
			expect(sharesApi.createShare).toHaveBeenCalledWith("prj_1", "guide.md"),
		);
	});

	it("shows the lock holder banner when another user holds the page", async () => {
		mockGuidePage();
		const holder = {
			path: "guide.md",
			user_id: "u2",
			username: "alice",
			acquired_at: "2026-01-01T00:00:00Z",
			expires_at: "2026-01-01T00:05:00Z",
		};
		vi.mocked(locksApi.acquireLock).mockRejectedValue(
			Object.assign(new Error("held"), {
				status: 409,
				code: "page_locked",
				data: { lock: holder },
			}),
		);
		const user = userEvent.setup();
		renderPage("/projects/prj_1/docs/guide.md");

		await user.click(await screen.findByRole("button", { name: /解锁编辑/ }));
		expect(await screen.findByRole("button", { name: "强制解锁" })).toBeInTheDocument();
		// Still read-only: no editor.
		expect(getEditor()).toBeUndefined();
	});

	it("force-unlocks the page after confirmation", async () => {
		mockGuidePage();
		const holder = {
			path: "guide.md",
			user_id: "u2",
			username: "alice",
			acquired_at: "2026-01-01T00:00:00Z",
			expires_at: "2026-01-01T00:05:00Z",
		};
		vi.mocked(locksApi.acquireLock).mockRejectedValue(
			Object.assign(new Error("held"), {
				status: 409,
				code: "page_locked",
				data: { lock: holder },
			}),
		);
		vi.mocked(locksApi.forceReleaseLock).mockResolvedValue({ released: true });
		const user = userEvent.setup();
		renderPage("/projects/prj_1/docs/guide.md");

		await user.click(await screen.findByRole("button", { name: /解锁编辑/ }));
		await screen.findByRole("button", { name: "强制解锁" });

		const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
		await user.click(screen.getByRole("button", { name: "强制解锁" }));
		await new Promise((r) => setTimeout(r, 100));
		expect(confirmSpy).toHaveBeenCalled();
		expect(locksApi.forceReleaseLock).toHaveBeenCalledWith(
			"prj_1",
			"guide.md",
		);
		expect(screen.queryByRole("button", { name: "强制解锁" })).toBeNull();
	});

	it("commits with Cmd+S without leaving edit mode", async () => {
		mockGuidePage();
		vi.mocked(changesetsApi.getRevision).mockResolvedValue({
			revision: "rev1",
		});
		vi.mocked(changesetsApi.submitChangeset).mockResolvedValue({
			commit: "c1",
			revision: "c1",
		});
		const user = userEvent.setup();
		vi.mocked(locksApi.releaseLock).mockClear();
		renderPage("/projects/prj_1/docs/guide.md");

		await user.click(await screen.findByRole("button", { name: /解锁编辑/ }));
		await screen.findAllByRole("textbox");
		const editor = getEditor() as HTMLElement;
		await user.click(editor);
		await user.keyboard("x");
		await new Promise((r) => setTimeout(r, 100));

		await user.keyboard("{Control>}s");
		await new Promise((r) => setTimeout(r, 50));
		await vi.waitFor(() =>
			expect(changesetsApi.submitChangeset).toHaveBeenCalled(),
		);
		// Cmd+S commits but stays in edit mode.
		expect(getEditor()).toBeTruthy();
		expect(locksApi.releaseLock).not.toHaveBeenCalled();
	});

	it("submits with a custom commit message", async () => {
		mockGuidePage();
		vi.mocked(changesetsApi.getRevision).mockResolvedValue({
			revision: "rev1",
		});
		vi.mocked(changesetsApi.submitChangeset).mockResolvedValue({
			commit: "c1",
			revision: "c1",
		});
		const user = userEvent.setup();
		renderPage("/projects/prj_1/docs/guide.md");

		await user.click(await screen.findByRole("button", { name: /解锁编辑/ }));
		await screen.findAllByRole("textbox");
		const editor = getEditor() as HTMLElement;
		await user.click(editor);
		await user.keyboard("x");
		await new Promise((r) => setTimeout(r, 100));

		await user.click(screen.getByRole("button", { name: "上锁并提交" }));
		await user.type(
			screen.getByLabelText("commit message"),
			"fix typo",
		);
		await user.click(screen.getByRole("button", { name: "提交并上锁" }));
		await vi.waitFor(() =>
			expect(changesetsApi.submitChangeset).toHaveBeenCalledWith(
				"prj_1",
				expect.objectContaining({ message: "fix typo" }),
			),
		);
	});
});
