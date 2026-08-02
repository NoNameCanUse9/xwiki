import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { Toaster } from "sonner";
import { FileRowActions, NewPageForm } from "./file-actions";
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

describe("NewPageForm", () => {
  beforeEach(() => vi.clearAllMocks());

  it("creates a page via changeset", async () => {
    vi.mocked(changesetsApi.getRevision).mockResolvedValue({ revision: "r1" });
    vi.mocked(changesetsApi.submitChangeset).mockResolvedValue({
      commit: "c1",
      revision: "c1",
    });
    const user = userEvent.setup();
    wrap(<NewPageForm projectId="prj_1" />);
    await user.click(screen.getByRole("button", { name: "新建页面" }));
    await user.type(screen.getByLabelText("新页面路径"), "docs/hello.md");
    await user.click(screen.getByRole("button", { name: "创建" }));
    await vi.waitFor(() =>
      expect(changesetsApi.submitChangeset).toHaveBeenCalledWith(
        "prj_1",
        expect.objectContaining({
          base_revision: "r1",
          message: "",
          changes: [
            {
              op: "create",
              path: "docs/hello.md",
              content: expect.stringContaining("# hello"),
            },
          ],
        }),
      ),
    );
  });

  it("rejects invalid paths", async () => {
    const user = userEvent.setup();
    wrap(<NewPageForm projectId="prj_1" />);
    await user.click(screen.getByRole("button", { name: "新建页面" }));
    await user.type(screen.getByLabelText("新页面路径"), "../evil.md");
    await user.click(screen.getByRole("button", { name: "创建" }));
    expect(changesetsApi.submitChangeset).not.toHaveBeenCalled();
  });
});

describe("FileRowActions", () => {
  beforeEach(() => vi.clearAllMocks());

  it("deletes a file after confirm", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    vi.mocked(changesetsApi.getRevision).mockResolvedValue({ revision: "r1" });
    vi.mocked(changesetsApi.submitChangeset).mockResolvedValue({
      commit: "c1",
      revision: "c1",
    });
    const onDeleted = vi.fn();
    const user = userEvent.setup();
    wrap(
      <FileRowActions projectId="prj_1" path="docs/a.md" onDeleted={onDeleted} />,
    );
    await user.click(screen.getByRole("button", { name: "删除 docs/a.md" }));
    await vi.waitFor(() =>
      expect(changesetsApi.submitChangeset).toHaveBeenCalledWith(
        "prj_1",
        expect.objectContaining({
          changes: [{ op: "delete", path: "docs/a.md" }],
        }),
      ),
    );
    expect(onDeleted).toHaveBeenCalled();
  });

  it("renames a file via move", async () => {
    vi.mocked(changesetsApi.getRevision).mockResolvedValue({ revision: "r1" });
    vi.mocked(changesetsApi.submitChangeset).mockResolvedValue({
      commit: "c1",
      revision: "c1",
    });
    const user = userEvent.setup();
    wrap(
      <FileRowActions projectId="prj_1" path="docs/a.md" onDeleted={vi.fn()} />,
    );
    await user.click(screen.getByRole("button", { name: "重命名 docs/a.md" }));
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
