// Gate C helper: render swatches for every direction and write per-direction contrast pairs.
//   node brand/scripts/palette-sheet.mjs -> brand/concepts/palette-<dir>.json + palette.png
import { writeFileSync, mkdirSync } from "node:fs";
import { Resvg } from "@resvg/resvg-js";
import { directions, build, semantic, contrastPairs } from "./palette.mjs";
import { oklchToHex } from "./color.mjs";

mkdirSync("brand/concepts", { recursive: true });
const hex = (c) => oklchToHex(...c).hex;
let y = 30;
let body = "";
for (const [id, d] of Object.entries(directions)) {
  const p = build(id);
  const s = semantic(p);
  writeFileSync(`brand/concepts/palette-${id}.json`, JSON.stringify(contrastPairs(s), null, 2));
  body += `<text x="20" y="${y + 18}" fill="#F2F4F7" font-family="Segoe UI" font-size="20" font-weight="700">${id} — ${d.label}</text>`;
  y += 30;
  for (const theme of ["dark", "light"]) {
    const t = s[theme];
    // mock screen: canvas, a card, talk button, receive chip, text
    const x0 = theme === "dark" ? 20 : 560;
    body += `<rect x="${x0}" y="${y}" width="520" height="240" rx="16" fill="${hex(t["bg.canvas"])}"/>`;
    body += `<rect x="${x0 + 20}" y="${y + 20}" width="300" height="90" rx="14" fill="${hex(t["bg.surface"])}" stroke="${hex(t["border.default"])}"/>`;
    body += `<text x="${x0 + 36}" y="${y + 52}" fill="${hex(t["fg.default"])}" font-family="Segoe UI" font-size="18" font-weight="700">Munte · 4 online</text>`;
    body += `<text x="${x0 + 36}" y="${y + 78}" fill="${hex(t["fg.muted"])}" font-family="Segoe UI" font-size="14">Ana: Plecăm la 9, ținem frecvența</text>`;
    body += `<text x="${x0 + 36}" y="${y + 98}" fill="${hex(t["accent.fg"])}" font-family="Segoe UI" font-size="13">@tu · editat</text>`;
    body += `<rect x="${x0 + 340}" y="${y + 20}" width="160" height="40" rx="20" fill="${hex(t["receive.solid"])}"/>`;
    body += `<text x="${x0 + 362}" y="${y + 46}" fill="${hex(t["fg.on-accent"])}" font-family="Segoe UI" font-size="15" font-weight="700">Bogdan vorbește</text>`;
    body += `<rect x="${x0 + 340}" y="${y + 70}" width="160" height="40" rx="20" fill="${hex(t["danger.solid"])}"/>`;
    body += `<text x="${x0 + 380}" y="${y + 96}" fill="${hex(t["fg.on-accent"])}" font-family="Segoe UI" font-size="15" font-weight="700">Ocupat</text>`;
    body += `<rect x="${x0 + 20}" y="${y + 130}" width="480" height="90" rx="45" fill="${hex(t["accent.solid"])}"/>`;
    body += `<text x="${x0 + 260}" y="${y + 184}" text-anchor="middle" fill="${hex(t["fg.on-accent"])}" font-family="Segoe UI" font-size="22" font-weight="800">Ține apăsat ca să vorbești</text>`;
    // ramps
    const ry = y + 250;
    ["neutral", "amber", "receive", "danger", "success"].forEach((r, ri) => {
      p[theme][r].forEach((c, i) => {
        body += `<rect x="${x0 + i * 43}" y="${ry + ri * 22}" width="41" height="20" fill="${hex(c)}"/>`;
      });
    });
  }
  y += 380;
}
const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1100" height="${y}"><rect width="100%" height="100%" fill="#0B0C0E"/>${body}</svg>`;
writeFileSync("brand/concepts/palette.png", new Resvg(svg, { font: { loadSystemFonts: true } }).render().asPng());
console.log("palette sheet written");
