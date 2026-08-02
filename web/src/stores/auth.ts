import { create } from "zustand";
import {
  login as apiLogin,
  logout as apiLogout,
  me as apiMe,
} from "@/lib/api/auth";
import type { User } from "@/lib/api/types";

interface AuthState {
  user: User | null;
  initializing: boolean;
  login: (username: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
  fetchMe: () => Promise<void>;
}

export const useAuthStore = create<AuthState>((set) => ({
  user: null,
  initializing: true,
  login: async (username, password) => {
    const res = await apiLogin(username, password);
    set({ user: res.user });
  },
  logout: async () => {
    await apiLogout();
    set({ user: null });
  },
  fetchMe: async () => {
    try {
      const res = await apiMe();
      set({ user: res.user, initializing: false });
    } catch {
      set({ user: null, initializing: false });
    }
  },
}));
