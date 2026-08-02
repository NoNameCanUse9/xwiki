import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Toaster } from "sonner";
import HomePage from "./home";
import * as projectsApi from "@/lib/api/projects";
import * as authStore from "@/stores/auth";
import type { Project } from "@/lib/api/types";

vi.mock("@/lib/api/projects", () => ({
  listProjects: vi.fn(),
  archiveProject: vi.fn(),
  createProject: vi.fn(),
}));

const sampleProject = (over: Partial<Project> = {}): Project => ({
  id: "prj_1",
  name: "docs-site",
  description: "产品文档",
  repo_dir: "repos/prj_1/repo.git",
  archived: false,
  created_at: "2026-08-02T12:00:00Z",
  updated_at: "2026-08-02T12:00:00Z",
  ...over,
});

function renderPage() {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <HomePage />
        <Toaster />
      </MemoryRouter>
    </QueryClientProvider>
  );
}

describe("HomePage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    authStore.useAuthStore.setState({
      user: { id: "usr_1", username: "admin", display_name: "Admin", is_admin: true },
      initializing: false,
      login: vi.fn(),
      logout: vi.fn(),
      fetchMe: vi.fn(),
    });
  });

  it("shows the empty state when there are no projects", async () => {
    vi.mocked(projectsApi.listProjects).mockResolvedValue({ projects: [] });
    renderPage();
    expect(await screen.findByText("还没有项目")).toBeInTheDocument();
  });

  it("renders active and archived project sections", async () => {
    vi.mocked(projectsApi.listProjects).mockResolvedValue({
      projects: [
        sampleProject({ id: "prj_1", name: "docs-site" }),
        sampleProject({ id: "prj_2", name: "legacy", archived: true }),
      ],
    });
    renderPage();
    expect(await screen.findByText("docs-site")).toBeInTheDocument();
    expect(screen.getByText("legacy")).toBeInTheDocument();
    expect(screen.getByText(/active · 1/)).toBeInTheDocument();
    expect(screen.getByText(/archived · 1/)).toBeInTheDocument();
  });

  it("archives a project and refreshes the list", async () => {
    vi.mocked(projectsApi.listProjects).mockResolvedValue({
      projects: [sampleProject()],
    });
    vi.mocked(projectsApi.archiveProject).mockResolvedValue({
      project: sampleProject({ archived: true }),
    });
    const user = userEvent.setup();
    renderPage();
    await user.click(await screen.findByRole("button", { name: /归档/ }));
    expect(projectsApi.archiveProject).toHaveBeenCalledWith("prj_1");
  });

  it("opens the create dialog and submits a new project", async () => {
    vi.mocked(projectsApi.listProjects).mockResolvedValue({ projects: [] });
    vi.mocked(projectsApi.createProject).mockResolvedValue({
      project: sampleProject(),
    });
    const user = userEvent.setup();
    renderPage();
    await user.click(screen.getByRole("button", { name: "新建项目" }));
    await user.type(await screen.findByLabelText("项目名"), "docs-site");
    await user.click(screen.getByRole("button", { name: "创建" }));
    expect(projectsApi.createProject).toHaveBeenCalledWith({
      name: "docs-site",
      description: "",
    });
  });
});
