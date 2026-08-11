import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Toaster } from "sonner";
import TokensPage from "./tokens";
import * as tokensApi from "@/lib/api/tokens";
import * as projectsApi from "@/lib/api/projects";

vi.mock("@/lib/api/tokens", () => ({
  listTokens: vi.fn(),
  createToken: vi.fn(),
  revokeToken: vi.fn(),
}));

vi.mock("@/lib/api/projects", () => ({
  listProjects: vi.fn(),
}));

const sampleProject = (over: Record<string, unknown> = {}) => ({
  id: "prj_1",
  name: "docs-site",
  description: "",
  repo_dir: "repos/prj_1/repo.git",
  archived: false,
  created_at: "2026-08-02T12:00:00Z",
  updated_at: "2026-08-02T12:00:00Z",
  ...over,
});

function renderPage() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <TokensPage />
        <Toaster />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("TokensPage", () => {
  beforeEach(() => vi.clearAllMocks());

  it("shows the empty state", async () => {
    vi.mocked(tokensApi.listTokens).mockResolvedValue({ tokens: [] });
    renderPage();
    expect(await screen.findByText("还没有 Token")).toBeInTheDocument();
  });

  it("creates a token from the project picker without path controls", async () => {
    vi.mocked(tokensApi.listTokens).mockResolvedValue({ tokens: [] });
    vi.mocked(projectsApi.listProjects).mockResolvedValue({
      projects: [
        sampleProject(),
        sampleProject({ id: "prj_2", name: "legacy" }),
      ],
    });
    vi.mocked(tokensApi.createToken).mockResolvedValue({
      token: {
        id: "tok_1",
        name: "ci",
        scope: "write",
        project_ids: ["prj_1", "prj_2"],
        created_at: "2026-08-02T12:00:00Z",
      },
      secret: "ad_secret123",
    });
    const user = userEvent.setup();
    renderPage();
    await user.type(await screen.findByLabelText("名称"), "ci");
    expect(screen.queryByLabelText("Scope")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "可访问项目" }));
    await user.click(screen.getByRole("option", { name: /docs-site/ }));
    await user.click(screen.getByRole("option", { name: /legacy/ }));
    expect(screen.getByText("已选择 2 个项目")).toBeInTheDocument();
    expect(screen.queryByText(/写入路径/)).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "创建 Token" }));
    expect(await screen.findByText("ad_secret123")).toBeInTheDocument();
    expect(tokensApi.createToken).toHaveBeenCalledWith({
      name: "ci",
      scope: "write",
      project_ids: ["prj_1", "prj_2"],
    });
  });

  it("requires confirmation in a web dialog before revoking", async () => {
    vi.mocked(tokensApi.listTokens).mockResolvedValue({
      tokens: [
        {
          id: "tok_1",
          name: "ci",
          scope: "write",
          project_ids: ["prj_1"],
          created_at: "2026-08-02T12:00:00Z",
        },
      ],
    });
    vi.mocked(projectsApi.listProjects).mockResolvedValue({
      projects: [sampleProject()],
    });
    vi.mocked(tokensApi.revokeToken).mockResolvedValue({ ok: true });
    const user = userEvent.setup();
    renderPage();
    await user.click(await screen.findByRole("button", { name: /撤销/ }));
    expect(tokensApi.revokeToken).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "确认撤销" }));
    await vi.waitFor(() =>
      expect(tokensApi.revokeToken).toHaveBeenCalledWith("tok_1"),
    );
  });
});
