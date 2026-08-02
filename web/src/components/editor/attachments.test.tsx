import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Toaster } from "sonner";
import AttachmentsPanel, { attachmentUrl } from "./attachments";
import * as changesetsApi from "@/lib/api/changesets";
import * as docsApi from "@/lib/api/docs";

vi.mock("@/lib/api/changesets", () => ({
  getRevision: vi.fn(),
  submitChangeset: vi.fn(),
}));

vi.mock("@/lib/api/docs", () => ({
  getTree: vi.fn(),
}));

function wrap() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <AttachmentsPanel projectId="prj_1" />
      <Toaster />
    </QueryClientProvider>
  );
}

describe("AttachmentsPanel", () => {
  beforeEach(() => vi.clearAllMocks());

  it("renders uploaded files with download links", async () => {
    vi.mocked(docsApi.getTree).mockResolvedValue({
      path: "attachments",
      tree: [
        { name: "logo.png", type: "blob", path: "attachments/logo.png" },
        { name: "note.txt", type: "blob", path: "attachments/note.txt" },
      ],
    });
    wrap();
    expect(await screen.findByText("logo.png")).toBeInTheDocument();
    expect(screen.getByText("note.txt")).toBeInTheDocument();
    expect(attachmentUrl("prj_1", "attachments/logo.png")).toBe(
      "/api/v1/projects/prj_1/attachments/attachments/logo.png",
    );
  });

  it("uploads a file as base64 changeset", async () => {
    vi.mocked(docsApi.getTree).mockResolvedValue({ path: "attachments", tree: [] });
    vi.mocked(changesetsApi.getRevision).mockResolvedValue({ revision: "r1" });
    vi.mocked(changesetsApi.submitChangeset).mockResolvedValue({
      commit: "c1",
      revision: "c1",
    });
    const user = userEvent.setup();
    wrap();
    const file = new File(["hello"], "note.txt", { type: "text/plain" });
    await user.upload(screen.getByLabelText("上传附件"), file);
    await vi.waitFor(() =>
      expect(changesetsApi.submitChangeset).toHaveBeenCalledWith(
        "prj_1",
        expect.objectContaining({
          base_revision: "r1",
          changes: [
            {
              op: "create",
              path: "attachments/note.txt",
              content: expect.any(String),
              encoding: "base64",
            },
          ],
        }),
      ),
    );
  });
});
