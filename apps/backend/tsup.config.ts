import { defineConfig } from "tsup";

export default defineConfig({
  entry: ["src/index.ts"],
  format: ["esm"],
  target: "node24",
  platform: "node",
  clean: true,
  sourcemap: true,
  // bundle the workspace protocol package; keep npm deps external
  noExternal: ["@titi/protocol"],
  // The demo bot's wasm engine (packages/core-wasm/pkg-node) is NOT imported
  // statically: demo-bot.ts loads it at runtime with createRequire from a path
  // next to dist/ (repo layout or /app/wasm in the image), so it stays outside
  // the bundle and its .wasm is read from disk.
});
