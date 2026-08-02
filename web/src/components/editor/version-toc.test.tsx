import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { extractToc, TocPanel, VersionPanel } from "./version-toc";
import * as historyApi from "@/lib/api/history";

vi.mock("@/lib/api/history", () => ({
  fileHistory: vi.fn(),
}));

function wrap(ui: React.ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>{ui}</MemoryRouter>
    </QueryClientProvider>
  );
}

describe("extractToc", () => {
  it("extracts h1-h3 with ids", () => {
    const root = document.createElement("div");
    root.innerHTML = "<h1>Title</h1><h2>Sub</h2><h4>Skip</h4><p>x</p>";
    const entries = extractToc(root);
    expect(entries.map((e) => e.text)).toEqual(["Title", "Sub"]);
    expect(entries[0].level).toBe(1);
    expect(entries[1].level).toBe(2);
    expect(root.querySelector("h1")?.id).toBeTruthy();
  });
});

describe("TocPanel", () => {
  it("renders entries and scrolls on click", async () => {
    const scrollIntoView = vi.fn();
    Element.prototype.scrollIntoView = scrollIntoView;
    wrap(
      <>
        <h1 id="toc-0">Title</h1>
        <h2 id="toc-1">Sub</h2>
        <TocPanel
          entries={[
            { id: "toc-0", text: "Title", level: 1 },
            { id: "toc-1", text: "Sub", level: 2 },
          ]}
        />
      </>,
    );
    expect(screen.queryAllByText("Title").length).toBeGreaterThanOrEqual(1);
    await userEvent.click(screen.queryAllByText("Sub")[1]);
    expect(scrollIntoView).toHaveBeenCalled();
  });
});

describe("VersionPanel", () => {
  beforeEach(() => vi.clearAllMocks());

  it("lists versions and calls onSelect", async () => {
    vi.mocked(historyApi.fileHistory).mockResolvedValue({
      path: "docs/a.md",
      commits: [
        { sha: "a".repeat(40), message: "first", author: "x", date: "2026-08-02T00:00:00Z" },
        { sha: "b".repeat(40), message: "second", author: "x", date: "2026-08-02T01:00:00Z" },
      ],
    });
    const onSelect = vi.fn();
    wrap(
      <VersionPanel
        projectId="prj_1"
        filePath="docs/a.md"
        currentVersion={null}
        onSelect={onSelect}
      />,
    );
    expect(await screen.findByText(/aaaaaaa/)).toBeInTheDocument();
    await userEvent.click(screen.getByText(/aaaaaaa/));
    expect(onSelect).toHaveBeenCalledWith("a".repeat(40));
    await userEvent.click(screen.getByText("最新版本"));
    expect(onSelect).toHaveBeenCalledWith(null);
  });
});
