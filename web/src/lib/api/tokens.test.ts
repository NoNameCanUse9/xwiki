import { afterEach, describe, expect, it, vi } from "vitest";
import { createToken, listTokens, revokeToken } from "./tokens";

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

describe("tokens api client", () => {
  it("lists tokens", async () => {
    mockFetchOnce(200, { tokens: [] });
    await listTokens();
    expect(fetch).toHaveBeenCalledWith("/api/v1/tokens", expect.anything());
  });

  it("creates a token with the exact payload", async () => {
    mockFetchOnce(201, {
      token: { id: "tok_1", name: "ci", scope: "write" },
      secret: "ad_abc",
    });
    const res = await createToken({
      name: "ci",
      scope: "write",
      project_ids: ["prj_1"],
      path_prefixes: ["docs"],
    });
    expect(res.secret).toBe("ad_abc");
    const [, init] = (fetch as ReturnType<typeof vi.fn>).mock.calls.at(-1) as [
      string,
      RequestInit,
    ];
    expect(JSON.parse(String(init.body))).toEqual({
      name: "ci",
      scope: "write",
      project_ids: ["prj_1"],
      path_prefixes: ["docs"],
    });
  });

  it("revokes a token via DELETE", async () => {
    mockFetchOnce(200, { ok: true });
    await revokeToken("tok_1");
    const [, init] = (fetch as ReturnType<typeof vi.fn>).mock.calls.at(-1) as [
      string,
      RequestInit,
    ];
    expect(init.method).toBe("DELETE");
  });
});
