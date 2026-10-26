import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/pwa",
  workers: 1,
  retries: 0,
  timeout: 30_000,
  use: {
    browserName: "chromium",
    // Headless Shell always reports notifications as denied. Use Chromium's
    // full implementation in headless mode so the real Notification API runs.
    channel: "chromium",
    baseURL: "http://localhost:8791",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
});
