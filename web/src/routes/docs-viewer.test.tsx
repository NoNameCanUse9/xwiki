import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import DocsViewerPage from "./docs-viewer";
import * as docsApi from "@/lib/api/docs";

vi.mock("@/lib/api/docs", () => ({
  getTree: vi.fn(),
  getPage: vi.fn(),
  getHome: vi.fn(),
}));

function renderPage(path = "/projects/prj_1/docs") {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter initialEntries={[path]}>
        <Routes>
          <Route path="/projects/:id/docs/*" element={<DocsViewerPage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>
  );
}

describe("DocsViewerPage", () => {
  beforeEach(() => vi.clearAllMocks());

  it("renders the project home at the docs root", async () => {
    vi.mocked(docsApi.getHome).mockResolvedValue({
      path: "README.md",
      format: "html",
      content: "<h1>docs-site</h1>",
    });
    vi.mocked(docsApi.getTree).mockResolvedValue({ path: "", tree: [] });
    renderPage();
    expect(await screen.findByText("docs-site")).toBeInTheDocument();
    expect(docsApi.getHome).toHaveBeenCalledWith("prj_1");
  });

  it("renders the tree and expands a directory lazily", async () => {
    vi.mocked(docsApi.getHome).mockResolvedValue({
      path: "README.md",
      format: "html",
      content: "<p>x</p>",
    });
    vi.mocked(docsApi.getTree)
      .mockResolvedValueOnce({
        path: "",
        tree: [
          { name: "docs", type: "tree", path: "docs" },
          { name: "README.md", type: "blob", path: "README.md" },
        ],
      })
      .mockResolvedValueOnce({
        path: "docs",
        tree: [{ name: "guide.md", type: "blob", path: "docs/guide.md" }],
      });
    const user = userEvent.setup();
    renderPage();
    await user.click(await screen.findByRole("button", { name: "docs" }));
    expect(await screen.findByRole("button", { name: "guide.md" })).toBeInTheDocument();
    expect(docsApi.getTree).toHaveBeenCalledWith("prj_1", "docs");
  });

  it("opens a file and shows breadcrumbs", async () => {
    vi.mocked(docsApi.getHome).mockResolvedValue({
      path: "README.md",
      format: "html",
      content: "<p>x</p>",
    });
    vi.mocked(docsApi.getTree).mockResolvedValue({
      path: "",
      tree: [{ name: "guide.md", type: "blob", path: "guide.md" }],
    });
    vi.mocked(docsApi.getPage).mockResolvedValue({
      path: "guide.md",
      format: "html",
      content: "<h1>Guide</h1>",
    });
    const user = userEvent.setup();
    renderPage();
    await user.click(await screen.findByRole("button", { name: "guide.md" }));
    expect(await screen.findByText("Guide")).toBeInTheDocument();
    expect(docsApi.getPage).toHaveBeenCalledWith("prj_1", "guide.md");
    // Breadcrumb shows the file segment.
    expect(screen.getAllByText("guide.md").length).toBeGreaterThan(0);
  });

  it("shows an error state when the page is missing", async () => {
    vi.mocked(docsApi.getPage).mockRejectedValue(new Error("missing"));
    vi.mocked(docsApi.getTree).mockResolvedValue({ path: "", tree: [] });
    renderPage("/projects/prj_1/docs/missing.md");
    expect(await screen.findByText("文档不存在")).toBeInTheDocument();
  });
});
