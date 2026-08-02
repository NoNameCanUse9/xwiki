import { afterEach, describe, expect, it, vi } from "vitest";
import {
  fileHistory,
  getCommitDiff,
  listCommits,
  revertCommit,
} from "./history";

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

function lastCall(): [string, RequestInit | undefined] {
  return (fetch as ReturnType<typeof vi.fn>).mock.calls.at(-1) as [
    string,
    RequestInit | undefined,
  ];
}

afterEach(() => vi.unstubAllGlobals());

describe("history api client", () => {
  it("lists commits with pagination", async () => {
    mockFetchOnce(200, { commits: [] });
    await listCommits("prj_1", 10, 5);
    expect(lastCall()[0]).toBe(
      "/api/v1/projects/prj_1/commits?limit=10&offset=5",
    );
  });

  it("fetches file history with encoded path", async () => {
    mockFetchOnce(200, { path: "docs/a.md", commits: [] });
    await fileHistory("prj_1", "docs/a.md");
    expect(lastCall()[0]).toBe(
      "/api/v1/projects/prj_1/files/history/docs/a.md",
    );
  });

  it("requests numstat by default and patch explicitly", async () => {
    mockFetchOnce(200, { sha: "s", format: "numstat", stats: [] });
    await getCommitDiff("prj_1", "s");
    expect(lastCall()[0]).toContain("?format=numstat");
    mockFetchOnce(200, { sha: "s", format: "patch", stats: [], patch: "" });
    await getCommitDiff("prj_1", "s", "patch");
    expect(lastCall()[0]).toContain("?format=patch");
  });

  it("posts a revert", async () => {
    mockFetchOnce(200, { commit: { sha: "new", message: "Revert" } });
    const res = await revertCommit("prj_1", "old");
    expect(res.commit.sha).toBe("new");
    expect(lastCall()[1]?.method).toBe("POST");
  });
});
