// UI tests of every screen and error state, in a real browser engine:
// WebKit on macOS (what Tauri uses there) and Chromium on Windows (what
// WebView2 is). The engine is scripted (see engine.ts); the real app is
// driven by e2e/app on the Windows runner. Runs only in GitHub Actions.

import { defineConfig, devices } from "@playwright/test";
import { testHome } from "../guard";

// Fails before anything starts if the stand-in home isn't safe.
testHome();

const browser = process.platform === "darwin" ? "webkit" : "chromium";
const device = browser === "webkit" ? devices["Desktop Safari"] : devices["Desktop Chrome"];

export default defineConfig({
  testDir: ".",
  testMatch: "*.spec.ts",
  globalSetup: "./global-setup.ts",
  forbidOnly: true,
  retries: 0,
  reporter: [["list"], ["html", { open: "never", outputFolder: "report" }]],
  outputDir: "results",
  use: {
    baseURL: "http://localhost:4173",
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
  },
  projects: [{ name: browser, use: { ...device, viewport: { width: 880, height: 700 } } }],
  // The built front end (npm run build), served as the app would load it.
  webServer: {
    command: "npx vite preview --port 4173 --strictPort",
    cwd: "../..",
    url: "http://localhost:4173",
    reuseExistingServer: false,
  },
});
