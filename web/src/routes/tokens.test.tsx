import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Toaster } from "sonner";
import TokensPage from "./tokens";
import * as tokensApi from "@/lib/api/tokens";

vi.mock("@/lib/api/tokens", () => ({
  listTokens: vi.fn(),
  createToken: vi.fn(),
  revokeToken: vi.fn(),
}));

function renderPage() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <TokensPage />
        <Toaster />
      </MemoryRouter>
    </QueryClientProvider>
  );
}

describe("TokensPage", () => {
  beforeEach(() => vi.clearAllMocks());

  it("shows the empty state", async () => {
    vi.mocked(tokensApi.listTokens).mockResolvedValue({ tokens: [] });
    renderPage();
    expect(await screen.findByText("还没有 Token")).toBeInTheDocument();
  });

  it("creates a token and shows the one-time secret", async () => {
    vi.mocked(tokensApi.listTokens).mockResolvedValue({ tokens: [] });
    vi.mocked(tokensApi.createToken).mockResolvedValue({
      token: {
        id: "tok_1",
        name: "ci",
        scope: "write",
        project_ids: ["prj_1"],
        path_prefixes: ["docs"],
        created_at: "2026-08-02T12:00:00Z",
      },
      secret: "ad_secret123",
    });
    const user = userEvent.setup();
    renderPage();
    await user.type(await screen.findByLabelText("名称"), "ci");
    await user.type(screen.getByLabelText("项目 ID（逗号分隔）"), "prj_1");
    await user.click(screen.getByRole("button", { name: "创建 Token" }));
    expect(await screen.findByText("ad_secret123")).toBeInTheDocument();
    expect(tokensApi.createToken).toHaveBeenCalledWith({
      name: "ci",
      scope: "write",
      project_ids: ["prj_1"],
      path_prefixes: ["docs"],
    });
  });

  it("revokes a token", async () => {
    vi.mocked(tokensApi.listTokens).mockResolvedValue({
      tokens: [
        {
          id: "tok_1",
          name: "ci",
          scope: "write",
          project_ids: ["prj_1"],
          path_prefixes: [],
          created_at: "2026-08-02T12:00:00Z",
        },
      ],
    });
    vi.mocked(tokensApi.revokeToken).mockResolvedValue({ ok: true });
    vi.spyOn(window, "confirm").mockReturnValue(true);
    const user = userEvent.setup();
    renderPage();
    await user.click(await screen.findByRole("button", { name: /撤销/ }));
    expect(tokensApi.revokeToken).toHaveBeenCalledWith("tok_1");
  });
});
