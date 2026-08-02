import { afterEach, describe, expect, it, vi } from "vitest";
import { importZip } from "./transfer";

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

describe("transfer api client", () => {
  it("posts an import with base64 files", async () => {
    mockFetchOnce(200, { commit: "c1", revision: "c1", imported: 2 });
    const res = await importZip("prj_1", "base", [
      { path: "a.md", content: "YQ==" },
    ]);
    expect(res.imported).toBe(2);
    const [, init] = (fetch as ReturnType<typeof vi.fn>).mock.calls.at(-1) as [
      string,
      RequestInit,
    ];
    expect(JSON.parse(String(init.body))).toEqual({
      base_revision: "base",
      message: "Import zip snapshot",
      files: [{ path: "a.md", content: "YQ==" }],
    });
  });
});
