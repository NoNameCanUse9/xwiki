import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { Toaster } from "sonner";
import AccountPage from "./account";
import * as authApi from "@/lib/api/auth";
import { useAuthStore } from "@/stores/auth";

vi.mock("@/lib/api/auth", () => ({
  changePassword: vi.fn(),
}));

function renderPage() {
  return render(
    <MemoryRouter>
      <AccountPage />
      <Toaster />
    </MemoryRouter>,
  );
}

describe("AccountPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAuthStore.setState({
      user: {
        id: "usr_1",
        username: "alice",
        display_name: "Alice",
        is_admin: false,
      },
    });
  });

  it("shows the current account info", async () => {
    renderPage();
    expect(await screen.findByText(/alice/)).toBeInTheDocument();
    expect(screen.getByText(/member/)).toBeInTheDocument();
    expect(screen.getByText("Alice")).toBeInTheDocument();
  });

  it("changes the password with current + new + confirmation", async () => {
    vi.mocked(authApi.changePassword).mockResolvedValue({ ok: true });
    const user = userEvent.setup();
    renderPage();
    await user.type(await screen.findByLabelText("当前密码"), "oldpass123");
    await user.type(screen.getByLabelText("新密码"), "newpass456");
    await user.type(screen.getByLabelText("确认新密码"), "newpass456");
    await user.click(screen.getByRole("button", { name: "保存" }));
    await vi.waitFor(() =>
      expect(authApi.changePassword).toHaveBeenCalledWith(
        "oldpass123",
        "newpass456",
      ),
    );
  });

  it("rejects mismatched confirmation without calling the API", async () => {
    const user = userEvent.setup();
    renderPage();
    await user.type(await screen.findByLabelText("当前密码"), "oldpass123");
    await user.type(screen.getByLabelText("新密码"), "newpass456");
    await user.type(screen.getByLabelText("确认新密码"), "different1");
    await user.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByText("两次输入的新密码不一致")).toBeInTheDocument();
    expect(authApi.changePassword).not.toHaveBeenCalled();
  });

  it("rejects passwords shorter than 8 characters", async () => {
    const user = userEvent.setup();
    renderPage();
    await user.type(await screen.findByLabelText("当前密码"), "oldpass123");
    await user.type(screen.getByLabelText("新密码"), "short");
    await user.type(screen.getByLabelText("确认新密码"), "short");
    await user.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByText("新密码至少 8 位")).toBeInTheDocument();
    expect(authApi.changePassword).not.toHaveBeenCalled();
  });
});
