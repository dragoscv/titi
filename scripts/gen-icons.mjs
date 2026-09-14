// Render PWA icons + Play store icon from apps/web/public/icon.svg.
// Run: pnpm dlx -p @resvg/resvg-js node scripts/gen-icons.mjs
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { Resvg } from "@resvg/resvg-js";

const svg = readFileSync(new URL("../apps/web/public/icon.svg", import.meta.url), "utf8");
const out = (name, size, padded = false) => {
  const src = padded ? svg.replace('viewBox="0 0 108 108"', 'viewBox="-14 -14 136 136"').replace('<rect width="108" height="108" rx="24"', '<rect x="-14" y="-14" width="136" height="136" rx="0"') : svg;
  const png = new Resvg(src, { fitTo: { mode: "width", value: size } }).render().asPng();
  writeFileSync(new URL(`../apps/web/public/${name}`, import.meta.url), png);
  return png;
};
out("icon-192.png", 192);
out("icon-512.png", 512);
out("icon-512-maskable.png", 512, true);
out("apple-icon.png", 180);
mkdirSync(new URL("../docs/store/", import.meta.url), { recursive: true });
writeFileSync(new URL("../docs/store/play-icon-512.png", import.meta.url), new Resvg(svg, { fitTo: { mode: "width", value: 512 } }).render().asPng());
console.log("icons written");
