import { describe, expect, it, vi, afterEach } from "vitest";
import { archiveProject, createProject, getProject, listProjects } from "./projects";

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

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("projects api client", () => {
  it("lists projects with credentials", async () => {
    mockFetchOnce(200, { projects: [] });
    await listProjects();
    expect(fetch).toHaveBeenCalledWith(
      "/api/v1/projects",
      expect.objectContaining({ credentials: "include" }),
    );
  });

  it("creates a project with JSON body", async () => {
    mockFetchOnce(201, {
      project: { id: "prj_1", name: "docs-site", archived: false },
    });
    const res = await createProject({ name: "docs-site", description: "x" });
    expect(res.project.name).toBe("docs-site");
    const [, init] = (fetch as ReturnType<typeof vi.fn>).mock.calls[0] as [
      string,
      RequestInit,
    ];
    expect(init.method).toBe("POST");
    expect(JSON.parse(String(init.body))).toEqual({
      name: "docs-site",
      description: "x",
    });
  });

  it("fetches one project by id", async () => {
    mockFetchOnce(200, { project: { id: "prj_1" } });
    const res = await getProject("prj_1");
    expect(res.project.id).toBe("prj_1");
    expect(fetch).toHaveBeenCalledWith(
      "/api/v1/projects/prj_1",
      expect.anything(),
    );
  });

  it("archives a project via POST", async () => {
    mockFetchOnce(200, { project: { id: "prj_1", archived: true } });
    const res = await archiveProject("prj_1");
    expect(res.project.archived).toBe(true);
    const [, init] = (fetch as ReturnType<typeof vi.fn>).mock.calls[0] as [
      string,
      RequestInit,
    ];
    expect(init.method).toBe("POST");
  });

  it("throws ApiError with the server code on 409", async () => {
    mockFetchOnce(409, {
      error: { code: "project_name_conflict", message: "exists" },
    });
    await expect(createProject({ name: "dup" })).rejects.toMatchObject({
      status: 409,
      code: "project_name_conflict",
    });
  });
});
