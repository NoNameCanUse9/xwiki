import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { router as appRouter } from "@/app/router";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Toaster } from "sonner";
import { useAuthStore } from "@/stores/auth";

function renderAt(path: string) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter(appRouter.routes, {
    initialEntries: [path],
  });
  return render(
    <QueryClientProvider client={qc}>
      <RouterProvider router={router} />
      <Toaster />
    </QueryClientProvider>,
  );
}

function mockMe(status: number, body: unknown) {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.includes("/api/v1/auth/me")) {
        return {
          ok: status >= 200 && status < 300,
          status,
          json: async () => body,
        } as Response;
      }
      return { ok: true, status: 200, json: async () => ({}) } as Response;
    }),
  );
}

describe("auth guard", () => {
  beforeEach(() => useAuthStore.setState({ user: null, initializing: true }));
  afterEach(() => vi.unstubAllGlobals());

  it("redirects unauthenticated users to /login", async () => {
    mockMe(401, { error: { code: "authentication_required" } });
    renderAt("/projects/prj_1/docs");
    expect(
      await screen.findByRole("heading", { name: /登录/i }),
    ).toBeInTheDocument();
  });

  it("keeps authenticated users on the page", async () => {
    mockMe(200, {
      user: {
        id: "usr_1",
        username: "admin",
        display_name: "Admin",
        is_admin: true,
      },
    });
    renderAt("/");
    expect(await screen.findByText("还没有项目")).toBeInTheDocument();
  });

  it("sends the session cookie on /auth/me", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: false,
      status: 401,
      json: async () => ({ error: { code: "authentication_required" } }),
    }));
    vi.stubGlobal("fetch", fetchMock);
    renderAt("/");
    await screen.findByRole("heading", { name: /登录/i });
    const [, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(init.credentials).toBe("include");
  });

  it("bounces logged-in users away from /login", async () => {
    mockMe(200, {
      user: {
        id: "usr_1",
        username: "admin",
        display_name: "Admin",
        is_admin: true,
      },
    });
    renderAt("/login");
    expect(await screen.findByText("还没有项目")).toBeInTheDocument();
  });
});
