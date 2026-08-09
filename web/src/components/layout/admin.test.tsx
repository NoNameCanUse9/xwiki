import { beforeEach, describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import AdminRoute from "./admin";
import { useAuthStore } from "@/stores/auth";

function renderRoute() {
  return render(
    <MemoryRouter initialEntries={["/admin"]}>
      <Routes>
        <Route path="/" element={<AdminRoute />}>
          <Route path="admin" element={<div>admin-content</div>} />
        </Route>
      </Routes>
    </MemoryRouter>,
  );
}

describe("AdminRoute", () => {
  beforeEach(() => {
    useAuthStore.setState({ user: null });
  });

  it("blocks non-admin users with a notice", () => {
    useAuthStore.setState({
      user: {
        id: "usr_1",
        username: "alice",
        display_name: "Alice",
        is_admin: false,
      },
    });
    renderRoute();
    expect(screen.getByText("无权限访问")).toBeInTheDocument();
    expect(screen.queryByText("admin-content")).not.toBeInTheDocument();
  });

  it("renders the page for admins", () => {
    useAuthStore.setState({
      user: {
        id: "usr_admin",
        username: "admin",
        display_name: "Admin",
        is_admin: true,
      },
    });
    renderRoute();
    expect(screen.getByText("admin-content")).toBeInTheDocument();
    expect(screen.queryByText("无权限访问")).not.toBeInTheDocument();
  });
});
