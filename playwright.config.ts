import { defineConfig } from "@playwright/test";

// E2E against the real Tauri app: start it with
//   WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222 npm run tauri dev
// then `npm run e2e`. Tests attach to its WebView2 over CDP (no browser download).
export default defineConfig({
  testDir: "e2e",
  workers: 1,
  timeout: 60_000,
  expect: { timeout: 15_000 },
  reporter: "list",
});
