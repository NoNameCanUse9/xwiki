import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import ProjectDetailPage from "./project-detail";
import * as projectsApi from "@/lib/api/projects";

vi.mock("@/lib/api/projects", () => ({
  getProject: vi.fn(),
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
  beforeEach(() => vi.clearAllMocks());

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
});
