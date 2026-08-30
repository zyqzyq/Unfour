import { defineConfig } from "vitest/config";
import { resolve } from "node:path";

export default defineConfig({
  resolve: {
    alias: {
      "@unfour/command-client": resolve(__dirname, "packages/command-client/src"),
      "@unfour/workspace-core": resolve(__dirname, "packages/workspace-core/src"),
      "@unfour/workspace-local": resolve(__dirname, "packages/workspace-local/src"),
      "@unfour/ui": resolve(__dirname, "packages/ui/src"),
    },
  },
  test: {
    include: [
      "packages/*/src/**/*.test.{ts,tsx}",
      "apps/*/src/**/*.test.{ts,tsx}",
      // Cross-module integration tests live outside src; *.test.ts only so
      // Playwright *.spec.ts files are not picked up by vitest.
      "packages/*/tests/integration/**/*.test.{ts,tsx}",
      "apps/*/tests/**/*.test.{ts,tsx}",
    ],
    // Node by default keeps the pure-logic suite fast. Component tests opt into
    // a DOM via a `// @vitest-environment jsdom` docblock at the top of the file.
    environment: "node",
    setupFiles: ["./vitest.setup.ts"],
    coverage: {
      provider: "v8",
      reporter: ["text", "html", "lcov", "json-summary"],
      reportsDirectory: "coverage",
      include: ["packages/*/src/**/*.{ts,tsx}", "apps/*/src/**/*.{ts,tsx}"],
      exclude: [
        "packages/*/src/**/*.test.{ts,tsx}",
        "apps/*/src/**/*.test.{ts,tsx}",
        "packages/*/src/**/index.ts",
        "packages/*/src/**/*.d.ts",
      ],
    },
  },
});
