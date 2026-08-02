import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { Toaster } from "sonner";
import ImportProjectDialog from "./import-project-dialog";
import * as transferApi from "@/lib/api/transfer";

vi.mock("@/lib/api/transfer", () => ({
  importRepo: vi.fn(),
  importZip: vi.fn(),
}));

function wrap() {
  return render(
    <MemoryRouter initialEntries={["/"]}>
      <Routes>
        <Route path="/" element={<ImportProjectDialog />} />
        <Route path="/projects/:id/docs" element={<div>docs-view</div>} />
      </Routes>
      <Toaster />
    </MemoryRouter>
  );
}

describe("ImportProjectDialog", () => {
  beforeEach(() => vi.clearAllMocks());

  it("imports a repo from URL and navigates to its docs", async () => {
    vi.mocked(transferApi.importRepo).mockResolvedValue({
      project: { id: "prj_9", name: "imported" },
      commits: 3,
    });
    const user = userEvent.setup();
    wrap();
    await user.click(screen.getByRole("button", { name: "导入项目" }));
    await user.type(screen.getByLabelText("项目名"), "imported");
    await user.type(
      screen.getByLabelText("Git 仓库 URL"),
      "https://github.com/x/y.git",
    );
    await user.click(screen.getByRole("button", { name: "导入" }));
    await vi.waitFor(() =>
      expect(transferApi.importRepo).toHaveBeenCalledWith(
        "imported",
        "https://github.com/x/y.git",
      ),
    );
    expect(await screen.findByText("docs-view")).toBeInTheDocument();
  });

  it("rejects empty input", async () => {
    const user = userEvent.setup();
    wrap();
    await user.click(screen.getByRole("button", { name: "导入项目" }));
    await user.click(screen.getByRole("button", { name: "导入" }));
    expect(transferApi.importRepo).not.toHaveBeenCalled();
    expect(await screen.findByText("项目名与 Git URL 必填")).toBeInTheDocument();
  });
});
