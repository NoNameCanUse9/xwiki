import { beforeEach, describe, expect, it, vi } from "vitest";
import * as authApi from "@/lib/api/auth";
import { useAuthStore } from "./auth";

vi.mock("@/lib/api/auth", () => ({
  login: vi.fn(),
  logout: vi.fn(),
  me: vi.fn(),
}));

const adminUser = {
  id: "usr_1",
  username: "admin",
  display_name: "admin",
  is_admin: true,
};

describe("useAuthStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAuthStore.setState({ user: null, initializing: true });
  });

  it("login sets the current user", async () => {
    vi.mocked(authApi.login).mockResolvedValue({ user: adminUser });
    await useAuthStore.getState().login("admin", "secret123");
    expect(useAuthStore.getState().user?.username).toBe("admin");
  });

  it("login failure leaves the user null", async () => {
    vi.mocked(authApi.login).mockRejectedValue(new Error("invalid credentials"));
    await expect(
      useAuthStore.getState().login("admin", "bad")
    ).rejects.toThrow();
    expect(useAuthStore.getState().user).toBeNull();
  });

  it("fetchMe restores the session", async () => {
    vi.mocked(authApi.me).mockResolvedValue({ user: adminUser });
    await useAuthStore.getState().fetchMe();
    expect(useAuthStore.getState().user?.username).toBe("admin");
    expect(useAuthStore.getState().initializing).toBe(false);
  });

  it("fetchMe without session clears user", async () => {
    vi.mocked(authApi.me).mockRejectedValue(new Error("unauthorized"));
    await useAuthStore.getState().fetchMe();
    expect(useAuthStore.getState().user).toBeNull();
    expect(useAuthStore.getState().initializing).toBe(false);
  });
});
