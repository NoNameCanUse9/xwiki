import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import ApiDocsPage from "./api-docs";

// The Scalar component is lazy-loaded; stub it to avoid pulling the real one.
vi.mock("@scalar/api-reference", () => ({
  ApiReference: () => <div data-testid="scalar-ref">api reference</div>,
}));

describe("ApiDocsPage", () => {
  it("renders the page shell with a link back", async () => {
    render(
      <MemoryRouter>
        <ApiDocsPage />
      </MemoryRouter>,
    );
    expect(screen.getByText("api · openapi 3.0")).toBeInTheDocument();
    expect(screen.getByText("workspace")).toBeInTheDocument();
    // Lazy chunk resolves.
    expect(await screen.findByTestId("scalar-ref")).toBeInTheDocument();
  });
});
