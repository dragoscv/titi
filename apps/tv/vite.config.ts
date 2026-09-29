import { defineConfig } from "vite";
import react, { reactCompilerPreset } from "@vitejs/plugin-react";
import babel from "@rolldown/plugin-babel";
import tailwindcss from "@tailwindcss/vite";
import { copyFileSync, mkdirSync } from "node:fs";
import { resolve } from "node:path";

// Packaged .wgt loads from a file-like origin: relative base, single chunk
// (no lazy chunk URLs), wasm inlined as an asset next to index.html.
// Tizen 9 = Chromium 120 (probe-verified WebCodecs Opus, AudioWorklet, oklch).
export default defineConfig({
  base: "./",
  plugins: [
    react(), babel({ presets: [reactCompilerPreset()] }),
    tailwindcss(),
    {
      name: "tizen-files",
      closeBundle() {
        const out = resolve(__dirname, "dist");
        mkdirSync(out, { recursive: true });
        for (const f of ["config.xml", "icon.png"]) copyFileSync(resolve(__dirname, "tizen", f), resolve(out, f));
      },
    },
  ],
  server: { port: 3143 },
  build: {
    target: "chrome108",
    outDir: "dist",
    emptyOutDir: true,
    assetsInlineLimit: 0,
    modulePreload: false,
    rollupOptions: { output: { inlineDynamicImports: true } },
  },
});
