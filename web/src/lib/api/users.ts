import { api } from "./client";

export interface UserView {
  id: string;
  username: string;
  display_name: string;
  is_admin: boolean;
  disabled: boolean;
  created_at: string;
}

export function listUsers() {
  return api<{ users: UserView[] }>("/users");
}

export function createUser(input: {
  username: string;
  password: string;
  display_name?: string;
  is_admin?: boolean;
}) {
  return api<{ user: UserView }>("/users", {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export function disableUser(id: string) {
  return api<{ user: UserView }>(`/users/${encodeURIComponent(id)}/disable`, {
    method: "POST",
  });
}

export function enableUser(id: string) {
  return api<{ user: UserView }>(`/users/${encodeURIComponent(id)}/enable`, {
    method: "POST",
  });
}

export function resetUserPassword(id: string, password: string) {
  return api<{ ok: boolean }>(`/users/${encodeURIComponent(id)}/password`, {
    method: "POST",
    body: JSON.stringify({ password }),
  });
}

export function deleteUser(id: string) {
  return api<{ ok: boolean }>(`/users/${encodeURIComponent(id)}`, {
    method: "DELETE",
  });
}
