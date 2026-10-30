// Brand v1 concept directions + comparison sheet (16..256 px, dark/light, Android mask,
// Windows taskbar strip, mono, inverse, blur squint). Run from repo root:
//   node brand/scripts/concepts.mjs   -> brand/concepts/*.svg + brand/concepts/sheet.png
import { mkdirSync, writeFileSync } from "node:fs";
import { Resvg } from "@resvg/resvg-js";

const P = { bg: "#0E1013", amber: "#FFB020", deep: "#D98E00", teal: "#2DD4BF", ink: "#0E1013", light: "#F2F4F7" };
const rad = (d) => (d * Math.PI) / 180;
const arc = (cx, cy, r, a0, a1) => {
  const p = (a) => `${(cx + r * Math.cos(rad(a))).toFixed(2)},${(cy + r * Math.sin(rad(a))).toFixed(2)}`;
  return `M${p(a0)} A${r},${r} 0 0,1 ${p(a1)}`;
};

// Each concept draws on the 108-unit Android adaptive canvas; art stays inside the 66-unit safe circle.
const concepts = {
  A: {
    name: "Chirp — bubble bird, one signal arc",
    art: (c) => `
      <circle cx="52" cy="52" r="21" fill="${c.amber}"/>
      <path d="M38,62 L52,72 L35,79 Z" fill="${c.amber}"/>
      <path d="M71,46 L82,52 L71,58 Z" fill="${c.deep}"/>
      <circle cx="59" cy="46" r="3.6" fill="${c.ink}"/>
      <path d="${arc(52, 52, 30, 215, 255)}" stroke="${c.teal}" stroke-width="5" stroke-linecap="round" fill="none"/>`,
  },
  B: {
    name: "Pebble — Talk-button squircle bird with speaker grille",
    art: (c) => `
      <rect x="31" y="31" width="44" height="44" rx="18" fill="${c.amber}"/>
      <path d="M74,47 L85,54 L74,61 Z" fill="${c.deep}"/>
      <circle cx="62" cy="45" r="3.8" fill="${c.ink}"/>
      <path d="M40,59 H54 M40,66 H50" stroke="${c.ink}" stroke-width="4" stroke-linecap="round"/>`,
  },
  C: {
    name: "t-bird — lowercase t standing as a bird, arc from the head",
    art: (c) => `
      <path d="M50,32 V64 Q50,77 63,77 H67" stroke="${c.amber}" stroke-width="12" stroke-linecap="round" fill="none"/>
      <path d="M37,47 H67" stroke="${c.amber}" stroke-width="11" stroke-linecap="round"/>
      <path d="M54,26 L66,32 L54,38 Z" fill="${c.deep}"/>
      <circle cx="49" cy="31" r="2.6" fill="${c.ink}"/>
      <path d="${arc(50, 32, 17, 200, 250)}" stroke="${c.teal}" stroke-width="4.5" stroke-linecap="round" fill="none"/>`,
  },
  D: {
    name: "Wave wing — round bird, wing cut as two sound waves",
    art: (c) => `
      <circle cx="52" cy="54" r="22" fill="${c.amber}"/>
      <path d="M36,66 L44,74 L33,78 Z" fill="${c.amber}"/>
      <path d="M72,48 L83,54 L72,60 Z" fill="${c.deep}"/>
      <circle cx="60" cy="47" r="3.6" fill="${c.ink}"/>
      <path d="${arc(56, 60, 8, 110, 200)} ${arc(56, 60, 15, 110, 200)}" stroke="${c.ink}" stroke-width="3.6" stroke-linecap="round" fill="none"/>`,
  },
};

const tile = (art, { bg = P.bg, rx = 24, vb = "0 0 108 108" } = {}) =>
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${vb}"><rect x="-20" y="-20" width="148" height="148" rx="${rx}" fill="${bg}"/>${art}</svg>`;
const palettes = {
  dark: P,
  light: { ...P, deep: "#B87400" },
  mono: { ...P, amber: "#FFFFFF", deep: "#FFFFFF", teal: "#FFFFFF", ink: P.bg },
  inverse: { ...P, amber: P.bg, deep: P.bg, teal: P.bg, ink: P.amber },
};
const png = (svg, size) => new Resvg(svg, { fitTo: { mode: "width", value: size } }).render().asPng();
const uri = (buf) => `data:image/png;base64,${buf.toString("base64")}`;
const img = (svg, size, x, y, draw = size, extra = "") =>
  `<image x="${x}" y="${y}" width="${draw}" height="${draw}" href="${uri(png(svg, size))}" ${extra}/>`;

mkdirSync("brand/concepts", { recursive: true });
const ROW = 300;
const W = 2140;
let body = "";
const neighbours = ["#2F6FEB", "#E5484D", "#30A46C", "#8E4EC6", "#F76B15"];
const nb = (x, y, s, col, i) =>
  `<rect x="${x}" y="${y}" width="${s}" height="${s}" rx="${s * 0.22}" fill="${col}"/>` +
  [`<circle cx="${x + s / 2}" cy="${y + s / 2}" r="${s * 0.22}" fill="#fff"/>`,
   `<rect x="${x + s * 0.3}" y="${y + s * 0.3}" width="${s * 0.4}" height="${s * 0.4}" fill="#fff"/>`,
   `<path d="M${x + s / 2},${y + s * 0.25} L${x + s * 0.75},${y + s * 0.72} L${x + s * 0.25},${y + s * 0.72} Z" fill="#fff"/>`][i % 3];

Object.entries(concepts).forEach(([id, c], row) => {
  writeFileSync(`brand/concepts/${id}.svg`, tile(c.art(P)));
  const y0 = 40 + row * ROW;
  const dark = tile(c.art(palettes.dark));
  const light = tile(c.art(palettes.light), { bg: P.light });
  body += `<text x="20" y="${y0 + 20}" fill="#F2F4F7" font-size="22" font-family="Segoe UI" font-weight="700">${id}</text>`;
  body += `<text x="20" y="${y0 + 46}" fill="#9AA3B2" font-size="13" font-family="Segoe UI">${c.name.split(" — ")[0]}</text>`;
  // dark + light true-pixel sizes
  let x = 160;
  for (const [svg, bgc] of [[dark, "#1E2229"], [light, "#FFFFFF"]]) {
    body += `<rect x="${x - 10}" y="${y0}" width="640" height="270" rx="12" fill="${bgc}"/>`;
    let xx = x;
    for (const s of [16, 24, 32, 48]) { body += img(svg, s, xx, y0 + 20); xx += s + 18; }
    body += img(svg, 128, xx, y0 + 20); xx += 146;
    body += img(svg, 256, xx, y0 + 6, 256);
    body += img(svg, 16, x, y0 + 120, 96, 'style="image-rendering:pixelated" image-rendering="optimizeSpeed"');
    body += `<text x="${x}" y="${y0 + 236}" fill="#9AA3B2" font-size="12" font-family="Segoe UI">16 px ×6</text>`;
    x += 660;
  }
  // Android circle mask (72 of 108 visible), 96 px
  body += `<defs><clipPath id="m${id}"><circle cx="${x + 48}" cy="${y0 + 68}" r="48"/></clipPath></defs>`;
  body += `<g clip-path="url(#m${id})">${img(tile(c.art(P), { rx: 0, vb: "18 18 72 72" }), 96, x, y0 + 20)}</g>`;
  body += `<text x="${x}" y="${y0 + 136}" fill="#9AA3B2" font-size="12" font-family="Segoe UI">Android mask</text>`;
  // taskbar strips 24 px (100 %) + 36 px (150 %)
  const tx = x + 120;
  body += `<rect x="${tx}" y="${y0 + 10}" width="300" height="44" rx="6" fill="#202020"/>`;
  body += `<rect x="${tx}" y="${y0 + 64}" width="300" height="60" rx="6" fill="#F3F3F3"/>`;
  neighbours.forEach((col, i) => {
    body += nb(tx + 10 + i * 40, y0 + 20, 24, col, i);
    body += i === 2 ? img(dark, 24, tx + 10 + 5 * 40, y0 + 20) : "";
  });
  neighbours.slice(0, 4).forEach((col, i) => { body += nb(tx + 10 + i * 50, y0 + 76, 36, col, i); });
  body += img(dark, 36, tx + 210, y0 + 76);
  body += `<text x="${tx}" y="${y0 + 142}" fill="#9AA3B2" font-size="12" font-family="Segoe UI">taskbar 24 / 36 px</text>`;
  // mono, inverse, blur
  let mx = x;
  for (const [label, svg, extra] of [
    ["mono", tile(c.art(palettes.mono)), ""],
    ["inverse", tile(c.art(palettes.inverse), { bg: P.amber }), ""],
    ["squint", dark, 'filter="url(#blur)"'],
  ]) {
    body += img(svg, 72, mx, y0 + 160, 72, extra);
    body += `<text x="${mx}" y="${y0 + 250}" fill="#9AA3B2" font-size="12" font-family="Segoe UI">${label}</text>`;
    mx += 100;
  }
});

const sheet = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${40 + 4 * ROW}" viewBox="0 0 ${W} ${40 + 4 * ROW}">
<defs><filter id="blur"><feGaussianBlur stdDeviation="2.5"/></filter></defs>
<rect width="100%" height="100%" fill="#0E1013"/>${body}</svg>`;
writeFileSync("brand/concepts/sheet.png", new Resvg(sheet, { font: { loadSystemFonts: true } }).render().asPng());
console.log("sheet written");
