import { defineConfig } from "@playwright/test";

// Chromium only: Playwright's WebKit build needs libicu 74, libflite 1, libjxl 0.8 and libbacktrace,
// which this (non-Ubuntu) host does not provide; see the M2 final-fix report.
export default defineConfig({
  testDir: "./e2e",
  use: { baseURL: "http://127.0.0.1:1420", viewport: { width: 1000, height: 680 }, colorScheme: "dark" },
  webServer: { command: "pnpm dev:mock", url: "http://127.0.0.1:1420", reuseExistingServer: true, timeout: 60_000 },
});
