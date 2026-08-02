import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { Toaster } from "sonner";
import FileMenu from "./file-menu";
import * as changesetsApi from "@/lib/api/changesets";

vi.mock("@/lib/api/changesets", () => ({
  getRevision: vi.fn(),
  submitChangeset: vi.fn(),
}));

function wrap(ui: React.ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>{ui}</MemoryRouter>
      <Toaster />
    </QueryClientProvider>
  );
}

const baseItems = {
  onEdit: vi.fn(),
  onToggleHistory: vi.fn(),
  onToggleAttachments: vi.fn(),
  onToggleBacklinks: vi.fn(),
};

describe("FileMenu", () => {
  beforeEach(() => vi.clearAllMocks());

  it("opens on ⋯ and shows all actions", async () => {
    const user = userEvent.setup();
    wrap(<FileMenu projectId="prj_1" filePath="docs/a.md" items={baseItems} />);
    await user.click(screen.getByRole("button", { name: "文件操作" }));
    expect(screen.getByRole("menuitem", { name: /编辑/ })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: /历史版本/ })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: /附件/ })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: /反向链接/ })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: /复制链接/ })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: /复制路径/ })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: /重命名/ })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: /删除/ })).toBeInTheDocument();
  });

  it("toggles panels via menu items", async () => {
    const user = userEvent.setup();
    wrap(<FileMenu projectId="prj_1" filePath="docs/a.md" items={baseItems} />);
    await user.click(screen.getByRole("button", { name: "文件操作" }));
    await user.click(screen.getByRole("menuitem", { name: /编辑/ }));
    expect(baseItems.onEdit).toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "文件操作" }));
    await user.click(screen.getByRole("menuitem", { name: /附件/ }));
    expect(baseItems.onToggleAttachments).toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "文件操作" }));
    await user.click(screen.getByRole("menuitem", { name: /反向链接/ }));
    expect(baseItems.onToggleBacklinks).toHaveBeenCalled();
  });

  it("copies the link and the path", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    const user = userEvent.setup();
    wrap(<FileMenu projectId="prj_1" filePath="docs/a.md" items={baseItems} />);
    await user.click(screen.getByRole("button", { name: "文件操作" }));
    await user.click(screen.getByRole("menuitem", { name: /复制链接/ }));
    await new Promise((r) => setTimeout(r, 100));
    expect(screen.queryByText("已复制链接")).not.toBeNull();
    await user.click(screen.getByRole("button", { name: "文件操作" }));
    await user.click(screen.getByRole("menuitem", { name: /复制路径/ }));
    await new Promise((r) => setTimeout(r, 100));
    expect(screen.queryByText("已复制路径")).not.toBeNull();
  });

  it("deletes after confirm and renames via move", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    vi.mocked(changesetsApi.getRevision).mockResolvedValue({ revision: "r1" });
    vi.mocked(changesetsApi.submitChangeset).mockResolvedValue({
      commit: "c1",
      revision: "c1",
    });
    const user = userEvent.setup();
    wrap(<FileMenu projectId="prj_1" filePath="docs/a.md" items={baseItems} />);
    await user.click(screen.getByRole("button", { name: "文件操作" }));
    await user.click(screen.getByRole("menuitem", { name: /删除/ }));
    await vi.waitFor(() =>
      expect(changesetsApi.submitChangeset).toHaveBeenCalledWith(
        "prj_1",
        expect.objectContaining({
          changes: [{ op: "delete", path: "docs/a.md" }],
        }),
      ),
    );

    await user.click(screen.getByRole("button", { name: "文件操作" }));
    await user.click(screen.getByRole("menuitem", { name: /重命名/ }));
    const input = screen.getByLabelText("重命名路径");
    await user.clear(input);
    await user.type(input, "docs/b.md");
    await user.keyboard("{Enter}");
    await vi.waitFor(() =>
      expect(changesetsApi.submitChangeset).toHaveBeenCalledWith(
        "prj_1",
        expect.objectContaining({
          changes: [{ op: "move", path: "docs/a.md", new_path: "docs/b.md" }],
        }),
      ),
    );
  });
});
