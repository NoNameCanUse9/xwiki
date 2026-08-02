import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Toaster } from "sonner";
import ImportFilesButton from "./import-files";
import * as changesetsApi from "@/lib/api/changesets";
import * as transferApi from "@/lib/api/transfer";

vi.mock("@/lib/api/changesets", () => ({
  getRevision: vi.fn(),
}));
vi.mock("@/lib/api/transfer", () => ({
  importZip: vi.fn(),
}));

function wrap() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <ImportFilesButton projectId="prj_1" />
      <Toaster />
    </QueryClientProvider>
  );
}

describe("ImportFilesButton", () => {
  beforeEach(() => vi.clearAllMocks());

  it("imports selected files into a single changeset commit", async () => {
    vi.mocked(changesetsApi.getRevision).mockResolvedValue({ revision: "r1" });
    vi.mocked(transferApi.importZip).mockResolvedValue({
      commit: "c1",
      revision: "c1",
      imported: 2,
    });
    const user = userEvent.setup();
    wrap();
    const input = screen.getByLabelText("导入文件夹");
    const fileA = new File(["# A\n"], "docs/a.md", { type: "text/markdown" });
    const fileB = new File(["# B\n"], "docs/sub/b.md", { type: "text/markdown" });
    Object.defineProperty(fileA, "webkitRelativePath", { value: "docs/a.md" });
    Object.defineProperty(fileB, "webkitRelativePath", { value: "docs/sub/b.md" });
    await user.upload(input, [fileA, fileB]);
    await vi.waitFor(() =>
      expect(transferApi.importZip).toHaveBeenCalledWith(
        "prj_1",
        "r1",
        expect.arrayContaining([
          expect.objectContaining({ path: "docs/a.md" }),
          expect.objectContaining({ path: "docs/sub/b.md" }),
        ]),
      ),
    );
  });
});
