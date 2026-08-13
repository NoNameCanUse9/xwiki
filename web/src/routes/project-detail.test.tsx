import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import ProjectDetailPage from "./project-detail";
import * as projectsApi from "@/lib/api/projects";
import * as docsApi from "@/lib/api/docs";
import * as historyApi from "@/lib/api/history";

vi.mock("@/lib/api/projects", () => ({
  getProject: vi.fn(),
}));

vi.mock("@/lib/api/docs", () => ({ getHome: vi.fn() }));
vi.mock("@/lib/api/history", () => ({
	listCommits: vi.fn(),
	getCommitDiff: vi.fn(),
	revertCommit: vi.fn(),
}));

function renderPage(projectId = "prj_1") {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter initialEntries={[`/projects/${projectId}`]}>
        <Routes>
          <Route path="/projects/:id" element={<ProjectDetailPage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>
  );
}

describe("ProjectDetailPage", () => {
  beforeEach(() => {
		vi.clearAllMocks();
		vi.mocked(docsApi.getHome).mockResolvedValue({
			path: "README.md", format: "html", content: "", revision: "r1",
		});
		vi.mocked(historyApi.listCommits).mockResolvedValue({ commits: [], has_more: false });
	});

  it("renders project metadata", async () => {
    vi.mocked(projectsApi.getProject).mockResolvedValue({
      project: {
        id: "prj_1",
        name: "docs-site",
        description: "产品文档",
        repo_dir: "repos/prj_1/repo.git",
        archived: false,
        created_at: "2026-08-02T12:00:00Z",
        updated_at: "2026-08-02T12:00:00Z",
      },
    });
    renderPage();
    expect(await screen.findByText("docs-site")).toBeInTheDocument();
    expect(screen.getByText("产品文档")).toBeInTheDocument();
    expect(screen.getByText("repos/prj_1/repo.git")).toBeInTheDocument();
  });

  it("shows the error state for a missing project", async () => {
    vi.mocked(projectsApi.getProject).mockRejectedValue(
      new Error("project not found"),
    );
    renderPage("prj_missing");
    expect(
      await screen.findByText("项目不存在或已被移除"),
    ).toBeInTheDocument();
  });

	it("searches full commit history and loads the next page", async () => {
		vi.mocked(projectsApi.getProject).mockResolvedValue({
			project: {
				id: "prj_1", name: "docs-site", description: "", repo_dir: "repos/prj_1/repo.git",
				archived: false, created_at: "2026-08-02T12:00:00Z", updated_at: "2026-08-02T12:00:00Z",
			},
		});
		vi.mocked(historyApi.listCommits)
			.mockResolvedValueOnce({ commits: [], has_more: false })
			.mockResolvedValueOnce({
				commits: [{ sha: "a".repeat(40), message: "fix release", author: "admin", date: "2026-08-02T12:00:00Z" }],
				has_more: true,
			})
			.mockResolvedValueOnce({
				commits: [{ sha: "b".repeat(40), message: "fix release older", author: "admin", date: "2026-08-01T12:00:00Z" }],
				has_more: false,
			});
		vi.mocked(historyApi.getCommitDiff).mockResolvedValue({ sha: "a", format: "numstat", stats: [] });
		const user = userEvent.setup();
		renderPage();
		const input = await screen.findByLabelText("搜索提交历史");
		await user.type(input, "release");
		// Debounced (300 ms) commit search fires without a submit button.
		await vi.waitFor(() => expect(historyApi.listCommits).toHaveBeenCalledWith("prj_1", 5, 0, "release"));
		expect(await screen.findByText("fix release")).toBeInTheDocument();
		await user.click(screen.getByRole("button", { name: "加载更多" }));
		await vi.waitFor(() => expect(historyApi.listCommits).toHaveBeenCalledWith("prj_1", 5, 1, "release"));
		expect(await screen.findByText("fix release older")).toBeInTheDocument();
	});
});
