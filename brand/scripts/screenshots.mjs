// Store screenshots: raw device captures (docs/store/screenshots/raw/<device>/*.png) -> captioned,
// brand-framed images per locale. Run: node brand/scripts/screenshots.mjs [device=a51] [WxH=1080x1920]
import { readdirSync, readFileSync, existsSync } from "node:fs";
import { Resvg } from "@resvg/resvg-js";
import { out, finish } from "./io.mjs";
import { C, place, markElements } from "./mark.mjs";

const [device = "a51", dims = "1080x1920"] = process.argv.slice(2).filter((a) => !a.startsWith("--"));
const [W, H] = dims.split("x").map(Number);
const FONT = { fontFiles: ["brand/fonts/Manrope-ExtraBold.ttf", "brand/fonts/Manrope-Medium.ttf"], loadSystemFonts: false, defaultFontFamily: "Manrope" };
const CAPTIONS = {
  "01-home": { en: ["Your radio,", "no signal needed"], ro: ["Stația ta,", "fără semnal"] },
  "02-group": { en: ["Everyone nearby", "in one group"], ro: ["Toți cei din jur,", "într-un grup"] },
  "03-talking": { en: ["Hold to talk.", "Release to listen."], ro: ["Ține apăsat și vorbește.", "Lasă și ascultă."] },
  "04-invite": { en: ["Invite with three", "words or a QR code"], ro: ["Invită cu trei", "cuvinte sau un cod QR"] },
  "05-chat": { en: ["Reply, react, edit —", "even offline"], ro: ["Răspunde, reacționează,", "editează — și offline"] },
};
const dir = `docs/store/screenshots/raw/${device}`;
if (!existsSync(dir)) throw new Error(`no raw captures in ${dir}`);
for (const f of readdirSync(dir).filter((n) => n.endsWith(".png"))) {
  const id = f.replace(/\.png$/, "");
  const cap = CAPTIONS[id];
  if (!cap) continue;
  const raw = readFileSync(`${dir}/${f}`);
  const rw = raw.readUInt32BE(16);
  const rh = raw.readUInt32BE(20);
  for (const lang of ["en", "ro"]) {
    const top = H * 0.2;
    const sh = H - top - H * 0.04;
    const sw = (sh * rw) / rh;
    const x = (W - sw) / 2;
    const r = sw * 0.07;
    const fs = W * 0.062;
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">
<rect width="${W}" height="${H}" fill="${C.graphite}"/>
<g fill="none" stroke="#2A2F38" stroke-width="2">${[0.35, 0.55, 0.75].map((k) => `<circle cx="${W * 0.88}" cy="${top * 0.45}" r="${W * k * 0.5}"/>`).join("")}</g>
${place(markElements(), W * 0.88, top * 0.45, W * 0.07)}
<text x="${W * 0.07}" y="${top * 0.42}" fill="${C.ink}" font-family="Manrope" font-weight="800" font-size="${fs}" letter-spacing="-1">${cap[lang][0]}</text>
<text x="${W * 0.07}" y="${top * 0.42 + fs * 1.15}" fill="${C.amber}" font-family="Manrope" font-weight="800" font-size="${fs}" letter-spacing="-1">${cap[lang][1]}</text>
<defs><clipPath id="s"><rect x="${x}" y="${top}" width="${sw}" height="${sh}" rx="${r}"/></clipPath></defs>
<rect x="${x - 6}" y="${top - 6}" width="${sw + 12}" height="${sh + 12}" rx="${r + 6}" fill="#2A2F38"/>
<image x="${x}" y="${top}" width="${sw}" height="${sh}" clip-path="url(#s)" href="data:image/png;base64,${raw.toString("base64")}"/>
</svg>`;
    out(`docs/store/screenshots/${lang}/${id}.png`, new Resvg(svg, { font: FONT }).render().asPng());
  }
}
finish("screenshots");
