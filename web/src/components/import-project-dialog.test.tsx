import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { Toaster } from "sonner";
import ImportProjectDialog from "./import-project-dialog";
import * as transferApi from "@/lib/api/transfer";

vi.mock("@/lib/api/transfer", () => ({
  importFolder: vi.fn(),
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
    </MemoryRouter>,
  );
}

function makeFile(path: string): File {
  const file = new File(["content"], path.split("/").pop()!, {
    type: "text/plain",
  });
  Object.defineProperty(file, "webkitRelativePath", {
    value: path,
    writable: true,
  });
  return file;
}

describe("ImportProjectDialog", () => {
  beforeEach(() => vi.clearAllMocks());

  it("imports a folder and navigates to its docs", async () => {
    vi.mocked(transferApi.importFolder).mockResolvedValue({
      project: { id: "prj_9", name: "imported" },
      commits: 3,
    });
    const user = userEvent.setup();
    wrap();
    await user.click(screen.getByRole("button", { name: "导入项目" }));
    const input = screen.getByLabelText<HTMLInputElement>("选择文件夹");
    const file = makeFile("my-folder/README.md");
    await user.upload(input, [file]);
    await user.click(screen.getByRole("button", { name: "导入" }));
    await vi.waitFor(() => expect(transferApi.importFolder).toHaveBeenCalled());
    expect(await screen.findByText("docs-view")).toBeInTheDocument();
  });

  it("rejects empty input", async () => {
    const user = userEvent.setup();
    wrap();
    await user.click(screen.getByRole("button", { name: "导入项目" }));
    await user.click(screen.getByRole("button", { name: "导入" }));
    expect(transferApi.importFolder).not.toHaveBeenCalled();
    expect(await screen.findByText("项目名必填")).toBeInTheDocument();
  });
});
