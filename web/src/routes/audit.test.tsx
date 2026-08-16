import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import AuditPage from "./audit";
import * as projectsApi from "@/lib/api/projects";
import * as auditApi from "@/lib/api/audit";

vi.mock("@/lib/api/projects", () => ({
  listProjects: vi.fn(),
}));

vi.mock("@/lib/api/audit", () => ({
  listAudit: vi.fn(),
}));

function renderPage() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <AuditPage />
      </MemoryRouter>
    </QueryClientProvider>
  );
}

describe("AuditPage", () => {
  beforeEach(() => vi.clearAllMocks());

  it("shows the no-project hint when there are no projects", async () => {
    vi.mocked(projectsApi.listProjects).mockResolvedValue({ projects: [] });
    renderPage();
    expect(await screen.findByText("暂无项目，无法查看审计日志。")).toBeInTheDocument();
  });

  it("lists audit entries for the first project", async () => {
    vi.mocked(projectsApi.listProjects).mockResolvedValue({
      projects: [
        {
          id: "prj_1",
          name: "demo",
          description: "",
          repo_dir: "/x",
          archived: false,
          created_at: "2026-08-01T00:00:00Z",
          updated_at: "2026-08-02T00:00:00Z",
        },
      ],
    });
    vi.mocked(auditApi.listAudit).mockResolvedValue({
      entries: [
        {
          id: "e1",
          actor_type: "user",
          actor_id: "admin",
          project_id: "prj_1",
          action: "doc.update",
          path: "docs/guide.md",
          created_at: "2026-08-02T12:00:00Z",
        },
      ],
      has_more: false,
    });
    renderPage();
    expect(await screen.findByText("doc.update")).toBeInTheDocument();
    expect(screen.getByText("docs/guide.md")).toBeInTheDocument();
    expect(screen.getByText(/admin/)).toBeInTheDocument();
    expect(auditApi.listAudit).toHaveBeenCalledWith("prj_1", 20, 0);
  });

  it("shows the empty state and supports switching projects", async () => {
    vi.mocked(projectsApi.listProjects).mockResolvedValue({
      projects: [
        {
          id: "prj_1",
          name: "alpha",
          description: "",
          repo_dir: "/a",
          archived: false,
          created_at: "2026-08-01T00:00:00Z",
          updated_at: "2026-08-02T00:00:00Z",
        },
        {
          id: "prj_2",
          name: "beta",
          description: "",
          repo_dir: "/b",
          archived: false,
          created_at: "2026-08-01T00:00:00Z",
          updated_at: "2026-08-02T00:00:00Z",
        },
      ],
    });
    vi.mocked(auditApi.listAudit).mockResolvedValue({ entries: [], has_more: false });
    const user = userEvent.setup();
    renderPage();
    expect(await screen.findByText("暂无审计记录")).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText("选择项目"), "prj_2");
    await new Promise((r) => setTimeout(r, 100));
    expect(auditApi.listAudit).toHaveBeenCalledWith("prj_2", 20, 0);
  });

  it("shows an error when the audit request fails", async () => {
    vi.mocked(projectsApi.listProjects).mockResolvedValue({
      projects: [
        {
          id: "prj_1",
          name: "demo",
          description: "",
          repo_dir: "/x",
          archived: false,
          created_at: "2026-08-01T00:00:00Z",
          updated_at: "2026-08-02T00:00:00Z",
        },
      ],
    });
    vi.mocked(auditApi.listAudit).mockRejectedValue(new Error("boom"));
    renderPage();
    expect(await screen.findByText("审计日志加载失败。")).toBeInTheDocument();
  });

  it("appends the next page when has_more and 加载更多 are clicked", async () => {
    vi.mocked(projectsApi.listProjects).mockResolvedValue({
      projects: [
        {
          id: "prj_1",
          name: "demo",
          description: "",
          repo_dir: "/x",
          archived: false,
          created_at: "2026-08-01T00:00:00Z",
          updated_at: "2026-08-02T00:00:00Z",
        },
      ],
    });
    const first = Array.from({ length: 20 }, (_, i) => ({
      id: `e${i}`,
      actor_type: "user",
      actor_id: "admin",
      project_id: "prj_1",
      action: "doc.update",
      path: `docs/${i}.md`,
      created_at: `2026-08-02T0${i % 10}:00:00Z`,
    }));
    vi.mocked(auditApi.listAudit)
      .mockResolvedValueOnce({ entries: first, has_more: true })
      .mockResolvedValueOnce({ entries: [{ id: "e20", actor_type: "user", actor_id: "admin", project_id: "prj_1", action: "doc.create", path: "docs/20.md", created_at: "2026-08-01T00:00:00Z" }], has_more: false });
    const user = userEvent.setup();
    renderPage();
    expect(await screen.findByText("加载更多")).toBeInTheDocument();
    await user.click(screen.getByText("加载更多"));
    expect(await screen.findByText("docs/20.md")).toBeInTheDocument();
    expect(auditApi.listAudit).toHaveBeenCalledTimes(2);
    expect(auditApi.listAudit).toHaveBeenLastCalledWith("prj_1", 20, 20);
    // No more pages → button disappears.
    expect(screen.queryByText("加载更多")).not.toBeInTheDocument();
  });
});
