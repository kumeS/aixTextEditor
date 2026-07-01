import { defineConfig } from "vitest/config";

// Unit-test config for the frontend's pure logic (slide grouping/derivation,
// store helpers, diff). The Tauri/React UI itself is not unit-tested here — these
// tests cover the framework-free functions that back the editor's behaviour, so
// they run fast in a plain Node environment with no DOM.
export default defineConfig({
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
