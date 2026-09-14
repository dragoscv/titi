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
});
