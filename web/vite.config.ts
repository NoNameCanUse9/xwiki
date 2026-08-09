/// <reference types="vitest/config" />
import { fileURLToPath, URL } from "node:url";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  server: {
    // WSL 镜像网络下 localhost 解析为 ::1 时，IPv6 回环无法从 Windows 侧转发，
    // 显式绑定 IPv4 回环保证 Windows 浏览器可通过 localhost:5173 访问。
    host: "127.0.0.1",
    proxy: {
      "/api": "http://localhost:9090",
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: "./src/test/setup.ts",
  },
});
