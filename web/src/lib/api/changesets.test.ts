import { afterEach, describe, expect, it, vi } from "vitest";
import { getRevision, submitChangeset } from "./changesets";

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

afterEach(() => vi.unstubAllGlobals());

describe("changesets api client", () => {
  it("fetches the current revision", async () => {
    mockFetchOnce(200, { revision: "abc123" });
    const res = await getRevision("prj_1");
    expect(res.revision).toBe("abc123");
  });

  it("posts a changeset with the exact payload", async () => {
    mockFetchOnce(200, { commit: "c1", revision: "c1" });
    await submitChangeset("prj_1", {
      base_revision: "abc",
      message: "update",
      changes: [{ op: "update", path: "docs/a.md", content: "# A" }],
    });
    const call = (fetch as ReturnType<typeof vi.fn>).mock.calls.at(-1) as [
      string,
      RequestInit,
    ];
    expect(call[0]).toBe("/api/v1/projects/prj_1/changesets");
    expect(call[1].method).toBe("POST");
    expect(JSON.parse(String(call[1].body))).toEqual({
      base_revision: "abc",
      message: "update",
      changes: [{ op: "update", path: "docs/a.md", content: "# A" }],
    });
  });

  it("posts a move changeset with the same payload by default", async () => {
    mockFetchOnce(200, { commit: "c1", revision: "c1" });
    await submitChangeset("prj_1", {
      base_revision: "abc",
      message: "",
      changes: [{ op: "move", path: "a.md", new_path: "b.md" }],
    });
    const call = (fetch as ReturnType<typeof vi.fn>).mock.calls.at(-1) as [
      string,
      RequestInit,
    ];
    expect(call[0]).toBe("/api/v1/projects/prj_1/changesets");
  });

  it("appends ?dry_run=true when dryRun is set", async () => {
    mockFetchOnce(200, {
      commit: "",
      revision: "abc",
      preview: { tree: "t", changes: [{ op: "move", path: "a.md", status: "moved" }] },
    });
    await submitChangeset(
      "prj_1",
      {
        base_revision: "abc",
        message: "",
        changes: [{ op: "move", path: "a.md", new_path: "b.md" }],
      },
      true,
    );
    const call = (fetch as ReturnType<typeof vi.fn>).mock.calls.at(-1) as [
      string,
      RequestInit,
    ];
    expect(call[0]).toBe("/api/v1/projects/prj_1/changesets?dry_run=true");
  });
});
