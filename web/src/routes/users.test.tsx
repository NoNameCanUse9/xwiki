import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Toaster } from "sonner";
import UsersPage from "./users";
import * as usersApi from "@/lib/api/users";

vi.mock("@/lib/api/users", () => ({
  listUsers: vi.fn(),
  createUser: vi.fn(),
  disableUser: vi.fn(),
  enableUser: vi.fn(),
}));

function renderPage() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <UsersPage />
        <Toaster />
      </MemoryRouter>
    </QueryClientProvider>
  );
}

const adminUser = {
  id: "usr_admin",
  username: "admin",
  display_name: "Admin",
  is_admin: true,
  disabled: false,
  created_at: "2026-08-02T12:00:00Z",
};

describe("UsersPage", () => {
  beforeEach(() => vi.clearAllMocks());

  it("lists users and creates a member", async () => {
    vi.mocked(usersApi.listUsers).mockResolvedValue({ users: [adminUser] });
    vi.mocked(usersApi.createUser).mockResolvedValue({
      user: { ...adminUser, id: "usr_2", username: "alice", display_name: "Alice" },
    });
    const user = userEvent.setup();
    renderPage();
    await user.type(await screen.findByLabelText("用户名"), "alice");
    await user.type(screen.getByLabelText("密码（至少 8 位）"), "password123");
    await user.click(screen.getByRole("button", { name: "创建用户" }));
    expect(usersApi.createUser).toHaveBeenCalledWith({
      username: "alice",
      password: "password123",
      display_name: undefined,
      is_admin: false,
    });
  });

  it("disables and enables a member", async () => {
    vi.mocked(usersApi.listUsers).mockResolvedValue({
      users: [
        adminUser,
        {
          id: "usr_2",
          username: "alice",
          display_name: "Alice",
          is_admin: false,
          disabled: false,
          created_at: "2026-08-02T12:00:00Z",
        },
      ],
    });
    vi.mocked(usersApi.disableUser).mockResolvedValue({
      user: { ...adminUser, id: "usr_2", username: "alice", disabled: true },
    });
    const user = userEvent.setup();
    renderPage();
    await user.click(await screen.findByRole("button", { name: /禁用/ }));
    expect(usersApi.disableUser).toHaveBeenCalledWith("usr_2");
  });
});
