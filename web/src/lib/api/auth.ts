import { api } from "./client";
import type { AuthResponse } from "./types";

export function login(username: string, password: string) {
  return api<AuthResponse>("/auth/login", {
    method: "POST",
    body: JSON.stringify({ username, password }),
  });
}

export function logout() {
  return api<{ ok: boolean }>("/auth/logout", { method: "POST" });
}

export function me() {
  return api<AuthResponse>("/auth/me");
}

export function changePassword(current_password: string, new_password: string) {
  return api<{ ok: boolean }>("/auth/password", {
    method: "POST",
    body: JSON.stringify({ current_password, new_password }),
  });
}
