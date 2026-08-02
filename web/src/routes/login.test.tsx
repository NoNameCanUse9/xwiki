import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import LoginPage from "./login";
import * as authApi from "@/lib/api/auth";

vi.mock("@/lib/api/auth", () => ({
  login: vi.fn(),
}));

function renderPage() {
  return render(
    <MemoryRouter initialEntries={["/login"]}>
      <Routes>
        <Route path="/login" element={<LoginPage />} />
        <Route path="/" element={<div>home-page</div>} />
      </Routes>
    </MemoryRouter>
  );
}

describe("LoginPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders username and password fields", () => {
    renderPage();
    expect(screen.getByLabelText("用户名")).toBeInTheDocument();
    expect(screen.getByLabelText("密码")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "登录" })).toBeInTheDocument();
  });

  it("shows an error alert when login fails", async () => {
    vi.mocked(authApi.login).mockRejectedValue(
      new Error("invalid credentials")
    );
    const user = userEvent.setup();
    renderPage();
    await user.type(screen.getByLabelText("用户名"), "admin");
    await user.type(screen.getByLabelText("密码"), "wrongpass");
    await user.click(screen.getByRole("button", { name: "登录" }));
    expect(await screen.findByRole("alert")).toBeInTheDocument();
  });

  it("navigates to home after successful login", async () => {
    vi.mocked(authApi.login).mockResolvedValue({
      user: {
        id: "usr_1",
        username: "admin",
        display_name: "admin",
        is_admin: true,
      },
    });
    const user = userEvent.setup();
    renderPage();
    await user.type(screen.getByLabelText("用户名"), "admin");
    await user.type(screen.getByLabelText("密码"), "secret123");
    await user.click(screen.getByRole("button", { name: "登录" }));
    expect(await screen.findByText("home-page")).toBeInTheDocument();
  });
});
