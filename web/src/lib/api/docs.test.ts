import { afterEach, describe, expect, it, vi } from "vitest";
import { getHome, getPage, getTree } from "./docs";

function mockFetchOnce(status: number, body: unknown) {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue({
      ok: status >= 200 && status < 300,
      status,
      json: async () => body,
    }),
  );
}

function lastUrl(): string {
  const call = (fetch as ReturnType<typeof vi.fn>).mock.calls.at(-1) as [
    string,
  ];
  return call[0];
}

afterEach(() => vi.unstubAllGlobals());

describe("docs api client", () => {
  it("lists the root tree without a path query", async () => {
    mockFetchOnce(200, { path: "", tree: [] });
    await getTree("prj_1");
    expect(lastUrl()).toBe("/api/v1/projects/prj_1/docs/tree");
  });

  it("encodes the directory path query", async () => {
    mockFetchOnce(200, { path: "docs", tree: [] });
    await getTree("prj_1", "docs/guide");
    expect(lastUrl()).toBe("/api/v1/projects/prj_1/docs/tree?path=docs%2Fguide");
  });

  it("requests html format by default and raw explicitly", async () => {
    mockFetchOnce(200, { path: "a.md", format: "html", content: "<h1>x</h1>" });
    await getPage("prj_1", "a.md");
    expect(lastUrl()).toContain("?format=html");
    mockFetchOnce(200, { path: "a.md", format: "raw", content: "# x" });
    await getPage("prj_1", "a.md", "raw");
    expect(lastUrl()).toContain("?format=raw");
  });

  it("fetches the project home", async () => {
    mockFetchOnce(200, { path: "README.md", format: "html", content: "" });
    await getHome("prj_1");
    expect(lastUrl()).toBe("/api/v1/projects/prj_1/docs/home");
  });
});
