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
    expect(screen.getByText("api · openapi 3.0")).toBeInTheDocument();
    expect(screen.getByText("workspace")).toBeInTheDocument();
    // The Scalar instance is created against the OpenAPI endpoint.
    await vi.waitFor(() =>
      expect(createApiReferenceMock).toHaveBeenCalledWith(
        expect.anything(),
        { url: "/api/openapi.json" },
      ),
    );
  });
});
