import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import JSZip from "jszip";
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

async function makeZip(): Promise<File> {
  const zip = new JSZip();
  zip.file("my-docs/README.md", "# hi");
  zip.file("my-docs/docs/guide.md", "# guide");
  const blob = await zip.generateAsync({ type: "blob" });
  return new File([blob], "my-docs.zip", { type: "application/zip" });
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

  it("imports a zip file, extracting entries and deriving the name", async () => {
    vi.mocked(transferApi.importFolder).mockResolvedValue({
      project: { id: "prj_9", name: "imported" },
      commits: 3,
    });
    const user = userEvent.setup();
    wrap();
    await user.click(screen.getByRole("button", { name: "导入项目" }));
    const zip = await makeZip();
    await user.upload(screen.getByLabelText("选择 zip 文件"), [zip]);
    expect(await screen.findByText(/已选择 2 个文件/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "导入" }));
    await vi.waitFor(() => expect(transferApi.importFolder).toHaveBeenCalled());
    const [name, , files] = vi.mocked(transferApi.importFolder).mock
      .calls[0] as [string, string, (File & { __relPath?: string })[]];
    expect(name).toBe("my-docs");
    expect(files.map((f) => f.__relPath)).toEqual([
      "README.md",
      "docs/guide.md",
    ]);
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
