import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import ApiDocsPage from "./api-docs";

// Scalar ships a Vue component; the page mounts it via createApiReference.
// Stub the module so tests do not pull Vue into the jsdom tree.
const createApiReferenceMock = vi.fn((_elementOrSelector: unknown, _configuration?: unknown) => ({
  destroy: vi.fn(),
}));

vi.mock("@scalar/api-reference", () => ({
  createApiReference: (
    elementOrSelector: unknown,
    configuration?: unknown,
  ) => createApiReferenceMock(elementOrSelector, configuration),
}));

describe("ApiDocsPage", () => {
  beforeEach(() => vi.clearAllMocks());

  it("renders the page shell with a link back", async () => {
    render(
      <MemoryRouter>
        <ApiDocsPage />
      </MemoryRouter>,
    );
    expect(screen.getByText("api · openapi 3.0.3")).toBeInTheDocument();
    expect(screen.getByText("workspace")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "切换到暗色模式" })).toBeInTheDocument();
    // The Scalar instance is created against the OpenAPI endpoint with the app theme.
    await vi.waitFor(() =>
      expect(createApiReferenceMock).toHaveBeenCalledWith(
        expect.anything(),
        expect.objectContaining({
          url: "/api/openapi.json",
          theme: "default",
          layout: "modern",
          showSidebar: true,
          showDeveloperTools: "always",
          darkMode: false,
          forceDarkModeState: "light",
          hideDarkModeToggle: true,
          customCss: expect.any(String),
        }),
      ),
    );
  });
});
