import { defineConfig, devices } from "@playwright/test";

// Starts ONLY what the suite needs: a local relay (tsx, no build) and the web app.
// Already-running servers are reused locally, so the edit→rerun loop is seconds.
//   E2E_WEB=dev    → next dev (hot reload; slower first page)
//   E2E_BASE_URL   → run against a deployed URL instead (prod smoke), no servers started
//   E2E_RELAY_URL  → relay the browsers use (default: the local one)
const RELAY_PORT = 18080;
const WEB_PORT = 3141;
const remote = process.env.E2E_BASE_URL;
const ci = !!process.env.CI;
export const RELAY_WS = process.env.E2E_RELAY_URL ?? `ws://127.0.0.1:${RELAY_PORT}/v1/ws`;

export default defineConfig({
  testDir: "./tests",
  fullyParallel: true,
  forbidOnly: ci,
  retries: ci ? 1 : 0,
  workers: ci ? 2 : undefined,
  timeout: 45_000,
  expect: { timeout: 10_000 },
  reporter: ci ? [["github"], ["html", { open: "never" }], ["junit", { outputFile: "test-results/junit.xml" }]] : [["list"], ["html", { open: "never" }]],
  use: {
    baseURL: remote ?? `http://127.0.0.1:${WEB_PORT}`,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
    // fake mic so getUserMedia + WebCodecs Opus run for real without a device
    launchOptions: { args: ["--use-fake-ui-for-media-stream", "--use-fake-device-for-media-stream"] },
    permissions: ["microphone", "clipboard-read", "clipboard-write"],
  },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "mobile", use: { ...devices["Pixel 7"] }, grep: /@smoke|@mobile/ },
  ],
  webServer: remote
    ? undefined
    : [
        {
          name: "relay",
          command: "pnpm --filter @titi/backend exec tsx src/index.ts",
          cwd: "..",
          url: `http://127.0.0.1:${RELAY_PORT}/health`,
          env: { PORT: String(RELAY_PORT), LOG_LEVEL: "info" },
          reuseExistingServer: !ci,
          stdout: "pipe",
          stderr: "pipe",
          timeout: 60_000,
        },
        {
          name: "web",
          command: process.env.E2E_WEB === "dev" ? "pnpm --filter @titi/web dev" : "pnpm --filter @titi/web start",
          cwd: "..",
          url: `http://127.0.0.1:${WEB_PORT}`,
          reuseExistingServer: !ci,
          stdout: "pipe",
          stderr: "pipe",
          timeout: 300_000, // cold next dev compile
        },
      ],
});
