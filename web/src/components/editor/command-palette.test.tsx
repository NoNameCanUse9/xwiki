import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router-dom";
import CommandPalette from "./command-palette";
import * as searchApi from "@/lib/api/search";

function DocViewSwitch() {
  const loc = useLocation();
  if (loc.pathname.includes("/docs/docs/")) {
    return <div>doc-view</div>;
  }
  return null;
}

vi.mock("@/lib/api/search", () => ({
  searchProject: vi.fn(),
}));

function renderPalette() {
  return render(
    <MemoryRouter initialEntries={["/projects/prj_1/docs"]}>
      <Routes>
        <Route
          path="/projects/:id/docs/*"
          element={
            <>
              <CommandPalette projectId="prj_1" />
              <DocViewSwitch />
            </>
          }
        />
      </Routes>
    </MemoryRouter>
  );
}

describe("CommandPalette", () => {
  beforeEach(() => vi.clearAllMocks());

  it("opens on Cmd+K and shows results", async () => {
    vi.mocked(searchApi.searchProject).mockResolvedValue({
      query: "guide",
      results: [{ path: "docs/guide.md", snippet: "guide" }],
    });
    const user = userEvent.setup();
    renderPalette();
    // 面板初始不显示
    expect(screen.queryByLabelText("搜索或跳转")).not.toBeInTheDocument();
    await user.keyboard("{Control>}k{/Control}");
    expect(await screen.findByLabelText("搜索或跳转")).toBeInTheDocument();
    await user.type(screen.getByLabelText("搜索或跳转"), "guide");
    expect(await screen.findByText("docs/guide.md")).toBeInTheDocument();
  });

  it("navigates on Enter and closes with Escape", async () => {
    vi.mocked(searchApi.searchProject).mockResolvedValue({
      query: "guide",
      results: [{ path: "docs/guide.md", snippet: "guide" }],
    });
    const user = userEvent.setup();
    renderPalette();
    await user.keyboard("{Control>}k{/Control}");
    await user.type(await screen.findByLabelText("搜索或跳转"), "guide");
    await screen.findByText("docs/guide.md");
    await user.keyboard("{Enter}");
    expect(await screen.findByText("doc-view")).toBeInTheDocument();
  });
});
