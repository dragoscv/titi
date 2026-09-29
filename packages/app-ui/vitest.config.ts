import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: ["test/**/*.test.ts"],
    environment: "node",
    coverage: {
      provider: "v8",
      include: ["src/**/*.ts"],
      exclude: ["src/**/*.tsx", "src/web/audio.ts", "src/web/host.ts", "src/**/index.ts"],
      reporter: ["text-summary", "lcov"],
      // ratchet: raise when coverage rises; lowering needs a reason in the commit message
      thresholds: { lines: 95, functions: 95, branches: 95, statements: 95 },
    },
  },
});
