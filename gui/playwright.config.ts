import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  use: { baseURL: "http://127.0.0.1:1420", viewport: { width: 1000, height: 680 }, colorScheme: "dark" },
  webServer: { command: "pnpm dev:mock", url: "http://127.0.0.1:1420", reuseExistingServer: true, timeout: 60_000 },
});
