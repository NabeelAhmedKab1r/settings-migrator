/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri loads the dev server on a fixed port (see src-tauri/tauri.conf.json).
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: "es2022" },
  // Unit tests only; the UI and app tests in e2e/ run in CI with their own runners.
  test: { include: ["src/**/*.test.ts"] },
});
