// Marketing + templates: feature graphic, OG, social, README header, A6 invite card, stickers,
// and the brand sheet (sizes, masks, taskbar, motion storyboard) used at the gates.
import { createRequire } from "node:module";
import { out } from "./io.mjs";
import { png } from "./icons.mjs";
import { C, tile, place, markElements, markParts, lockup, wordmark, sem, hex } from "./mark.mjs";
import { MOTION, PIVOTS } from "./motion-spec.mjs";

const require = createRequire(new URL("../../packages/app-ui/package.json", import.meta.url));
const QR = require("qrcode");
const T = {
  font: "Manrope",
  mono: "Geist Mono",
};

/** Lockup placed at x,y with given wordmark cap height (px). */
function lockupAt(x, y, h, { ink = C.ink, dot = C.amber } = {}) {
  const w = wordmark({ ink, dot });
  const H = w.top + w.bottom;
  const s = h / H;
  const r = h * 0.5;
  return place(markElements(), x + r, y + h / 2, r) + `<g transform="translate(${x + 2 * r + r * 0.45} ${y}) scale(${s}) translate(0 ${w.top})">${w.inner}</g>`;
}

const rings = (cx, cy, rs, col = "#2A2F38", sw = 1.5) => `<g fill="none" stroke="${col}" stroke-width="${sw}">${rs.map((r) => `<circle cx="${cx}" cy="${cy}" r="${r}"/>`).join("")}</g>`;

export function featureGraphic(lang = "en") {
  const copy = { en: ["Talk anywhere.", "No signal needed."], ro: ["Vorbește oriunde.", "Fără semnal."] }[lang];
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 500" width="1024" height="500">
<defs><radialGradient id="g" cx="76%" cy="52%" r="55%"><stop offset="0" stop-color="${C.amber}" stop-opacity="0.28"/><stop offset="1" stop-color="${C.graphite}" stop-opacity="0"/></radialGradient></defs>
<rect width="1024" height="500" fill="${C.graphite}"/><rect width="1024" height="500" fill="url(#g)"/>
${rings(780, 262, [120, 190, 260, 330])}
${place(markElements(), 780, 262, 150)}
${lockupAt(72, 96, 56)}
<text x="72" y="268" fill="${C.ink}" font-family="${T.font}" font-weight="800" font-size="54" letter-spacing="-1.5">${copy[0]}</text>
<text x="72" y="330" fill="${C.amber}" font-family="${T.font}" font-weight="800" font-size="54" letter-spacing="-1.5">${copy[1]}</text>
<text x="72" y="392" fill="${C.muted}" font-family="${T.font}" font-weight="500" font-size="22">${lang === "ro" ? "Wi‑Fi · hotspot · Bluetooth · criptat" : "Wi‑Fi · hotspot · Bluetooth · encrypted"}</text>
</svg>`;
}

export function ogImage(title, sub) {
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1200 630" width="1200" height="630">
<rect width="1200" height="630" fill="${C.graphite}"/>${rings(960, 330, [140, 220, 300, 380])}
${place(markElements(), 960, 330, 170)}
${lockupAt(80, 80, 64)}
<text x="80" y="330" fill="${C.ink}" font-family="${T.font}" font-weight="800" font-size="66" letter-spacing="-2">${title}</text>
<text x="80" y="410" fill="${C.amber}" font-family="${T.font}" font-weight="800" font-size="66" letter-spacing="-2">${sub}</text>
<text x="80" y="540" fill="${C.muted}" font-family="${T.mono}" font-size="24">titi.dragoscatalin.ro</text></svg>`;
}

const banner = (w, h) => `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${w} ${h}" width="${w}" height="${h}"><rect width="${w}" height="${h}" fill="${C.graphite}"/>${rings(w * 0.82, h / 2, [h * 0.3, h * 0.5, h * 0.7, h * 0.9])}${place(markElements(), w * 0.82, h / 2, h * 0.36)}${lockupAt(w * 0.06, h / 2 - h * 0.14, h * 0.28)}<text x="${w * 0.06}" y="${h / 2 + h * 0.3}" fill="${C.muted}" font-family="${T.font}" font-weight="600" font-size="${h * 0.09}">Talk anywhere. No signal needed.</text></svg>`;

function readmeHeader() {
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1280 320" width="1280" height="320"><rect width="1280" height="320" rx="24" fill="${C.graphite}"/>${rings(1080, 160, [90, 140, 190, 240])}${place(markElements(), 1080, 160, 100)}${lockupAt(72, 92, 72)}<text x="72" y="236" fill="${C.muted}" font-family="${T.font}" font-weight="600" font-size="30">Offline-first walkie-talkie · Android · iOS · Wear · TV · desktop · web</text></svg>`;
}

/** A6 printable invite card (105×148 mm) with a QR to the join page; group code is filled per event. */
async function inviteCard(lang) {
  const url = "https://titi.dragoscatalin.ro/join";
  const qr = await QR.toString(url, { type: "svg", margin: 0, errorCorrectionLevel: "M", color: { dark: "#0E1013", light: "#FFFFFF" } });
  const inner = qr.replace(/^.*?<svg[^>]*viewBox="([^"]+)"[^>]*>/s, (_, vb) => `<svg x="22.5" y="52" width="60" height="60" viewBox="${vb}">`);
  const c = {
    en: ["Join our radio", "1. Install Titi or open the link", "2. Scan the code", "3. Hold to talk", "Group code"],
    ro: ["Intră pe stația noastră", "1. Instalează Titi sau deschide linkul", "2. Scanează codul", "3. Ține apăsat ca să vorbești", "Codul grupului"],
  }[lang];
  return `<svg xmlns="http://www.w3.org/2000/svg" width="105mm" height="148mm" viewBox="0 0 105 148">
<rect width="105" height="148" fill="${C.graphite}"/>
${lockupAt(10, 10, 11)}
<text x="10" y="40" fill="${C.ink}" font-family="${T.font}" font-weight="800" font-size="8">${c[0]}</text>
<rect x="18.5" y="48" width="68" height="68" rx="6" fill="#FFFFFF"/>${inner}
<text x="10" y="126" fill="${C.muted}" font-family="${T.font}" font-weight="600" font-size="3.6">${c[1]}</text>
<text x="10" y="131" fill="${C.muted}" font-family="${T.font}" font-weight="600" font-size="3.6">${c[2]} · ${c[3]}</text>
<text x="10" y="140" fill="${C.amber}" font-family="${T.font}" font-weight="700" font-size="3.6">${c[4]}: ____ ____ ____</text>
</svg>`;
}

// ---------- stickers (512 px, transparent; WhatsApp/Telegram size) ----------
const STICKERS = {
  ok: () => `<g transform="translate(62 4)"><rect width="28" height="16" rx="8" fill="${hex(sem.dark["success.solid"])}"/><text x="14" y="11.5" text-anchor="middle" fill="${C.graphite}" font-family="${T.font}" font-weight="800" font-size="9">OK</text></g>`,
  roger: () => `<g transform="translate(58 4)"><rect width="38" height="16" rx="8" fill="${C.teal}"/><text x="19" y="11.5" text-anchor="middle" fill="${C.graphite}" font-family="${T.font}" font-weight="800" font-size="9">ROGER</text></g>`,
  sos: () => `<g transform="translate(60 4)"><rect width="32" height="16" rx="8" fill="${hex(sem.dark["emergency.solid"])}"/><text x="16" y="11.5" text-anchor="middle" fill="#FFFFFF" font-family="${T.font}" font-weight="800" font-size="9">SOS</text></g>`,
  hi: () => `<g transform="translate(62 4)"><rect width="28" height="16" rx="8" fill="${C.ink}"/><text x="14" y="11.5" text-anchor="middle" fill="${C.graphite}" font-family="${T.font}" font-weight="800" font-size="9">Hi!</text></g>`,
  omw: () => `<g transform="translate(52 4)"><rect width="44" height="16" rx="8" fill="${C.amber}"/><text x="22" y="11.5" text-anchor="middle" fill="${C.graphite}" font-family="${T.font}" font-weight="800" font-size="8">On my way</text></g>`,
  quiet: () => `<g transform="translate(60 4)"><rect width="32" height="16" rx="8" fill="${C.surface}" stroke="${C.muted}"/><text x="16" y="11.5" text-anchor="middle" fill="${C.ink}" font-family="${T.font}" font-weight="800" font-size="8">Shh</text></g>`,
};
function sticker(name) {
  const opts = { roger: { arcs: 2 }, quiet: { muted: true }, sos: { arcs: 2, colors: { arc: hex(sem.dark["emergency.solid"]) } } }[name] ?? {};
  const outline = markParts(opts)
    .map((p) => `<path d="${p.d}" fill="${p.stroke ? "none" : "#FFFFFF"}" stroke="#FFFFFF" stroke-width="${(p.width ?? 0) + 4.4}" stroke-linecap="round" stroke-linejoin="round"/>`)
    .join("");
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="512" height="512">${place(outline, 48, 58, 32)}${place(markElements(opts), 48, 58, 32)}${STICKERS[name](0)}</svg>`;
}

// ---------- brand sheet (gate evidence) ----------
function sheet() {
  const uri = (b) => `data:image/png;base64,${b.toString("base64")}`;
  const img = (svg, s, x, y, d = s, extra = "") => `<image x="${x}" y="${y}" width="${d}" height="${d}" href="${uri(png(svg, s))}" ${extra}/>`;
  let b = "";
  let x = 40;
  b += `<text x="40" y="44" fill="${C.ink}" font-family="${T.font}" font-weight="800" font-size="26">Titi brand v1 · Chirp</text>`;
  for (const s of [16, 20, 24, 32, 48, 64, 128, 256]) {
    b += img(tile(s), s, x, 80);
    b += `<text x="${x}" y="${80 + Math.max(s, 24) + 20}" fill="${C.muted}" font-family="${T.font}" font-size="12">${s}</text>`;
    x += s + 26;
  }
  // on light
  b += `<rect x="40" y="370" width="660" height="150" rx="14" fill="${hex(sem.light["bg.canvas"])}"/>`;
  x = 60;
  for (const s of [16, 24, 32, 48, 96]) {
    b += img(tile(s), s, x, 390);
    x += s + 24;
  }
  b += lockup({ ink: C.lightInk, dot: "#865900", colors: { beak: "#B87400" } }).replace("<svg ", '<svg x="330" y="395" width="350" height="100" ');
  // Android masks: circle, squircle, rounded square; themed mono
  const fgOnly = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="18 18 72 72"><rect x="0" y="0" width="108" height="108" fill="${C.graphite}"/>${place(markElements(), 54, 54, 30)}</svg>`;
  const mono = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="18 18 72 72"><rect width="108" height="108" fill="#2E3A2F"/>${place(markElements({ mono: true, colors: { body: "#C8E6C9", beak: "#C8E6C9", arc: "#C8E6C9" } }), 54, 54, 30)}</svg>`;
  const masks = [["circle", (x) => `<circle cx="${x + 48}" cy="128" r="48"/>`], ["squircle", (x) => `<rect x="${x}" y="80" width="96" height="96" rx="34"/>`], ["square", (x) => `<rect x="${x}" y="80" width="96" height="96" rx="18"/>`]];
  masks.forEach(([n, shape], i) => {
    const mx = 840 + i * 120;
    b += `<defs><clipPath id="m${i}">${shape(mx)}</clipPath></defs><g clip-path="url(#m${i})">${img(fgOnly, 96, mx, 80)}</g><text x="${mx}" y="196" fill="${C.muted}" font-family="${T.font}" font-size="12">${n}</text>`;
  });
  b += `<defs><clipPath id="mm"><circle cx="1248" cy="128" r="48"/></clipPath></defs><g clip-path="url(#mm)">${img(mono, 96, 1200, 80)}</g><text x="1200" y="196" fill="${C.muted}" font-family="${T.font}" font-size="12">themed</text>`;
  // taskbar strips
  const nb = ["#2F6FEB", "#E5484D", "#30A46C", "#8E4EC6", "#F76B15"];
  b += `<rect x="840" y="230" width="420" height="48" rx="8" fill="#202020"/><rect x="840" y="290" width="420" height="64" rx="8" fill="#F3F3F3"/>`;
  nb.forEach((c, i) => {
    b += `<rect x="${852 + i * 44}" y="242" width="24" height="24" rx="5" fill="${c}"/><rect x="${852 + i * 56}" y="304" width="36" height="36" rx="8" fill="${c}"/>`;
  });
  b += img(tile(24, { rx: 0.18, fill: 0.4 }), 24, 852 + 5 * 44, 242) + img(tile(36, { rx: 0.18, fill: 0.4 }), 36, 852 + 5 * 56, 304);
  // states
  [["idle", {}], ["talk", { arcs: 2 }], ["muted", { muted: true }]].forEach(([n, o], i) => {
    b += img(tile(64, o), 64, 840 + i * 100, 380) + `<text x="${840 + i * 100}" y="464" fill="${C.muted}" font-family="${T.font}" font-size="12">${n}</text>`;
  });
  // motion storyboard (intro, 8 frames) — sampled from the same keyframes
  b += `<text x="40" y="580" fill="${C.ink}" font-family="${T.font}" font-weight="800" font-size="18">intro · ${MOTION.logo.intro.$value.duration} ms</text>`;
  for (let i = 0; i < 8; i++) b += img(frame("intro", i / 7), 120, 40 + i * 140, 600);
  b += `<text x="40" y="760" fill="${C.ink}" font-family="${T.font}" font-weight="800" font-size="18">talking · searching · morph</text>`;
  [["talking", 0], ["talking", 0.5], ["searching", 0.2], ["searching", 0.6], ["morph", 0], ["morph", 0.5], ["morph", 1]].forEach(([n, t], i) => (b += img(frame(n, t), 120, 40 + i * 140, 780)));
  return `<svg xmlns="http://www.w3.org/2000/svg" width="1360" height="940"><rect width="100%" height="100%" fill="#0B0C0E"/>${b}</svg>`;
}

/** Render one frame of a logo animation at t (0..1) by linear sampling of the tracks (storyboard only). */
function frame(name, t) {
  const tracks = MOTION.logo[name].$value.tracks;
  const v = (keys, prop, def) => {
    const ks = keys.filter((k) => k[1] === prop);
    if (!ks.length) return def;
    if (t <= ks[0][0]) return ks[0][2];
    for (let i = 1; i < ks.length; i++) if (t <= ks[i][0]) return ks[i - 1][2] + ((t - ks[i - 1][0]) / (ks[i][0] - ks[i - 1][0])) * (ks[i][2] - ks[i - 1][2]);
    return ks[ks.length - 1][2];
  };
  const pivots = { ...PIVOTS, ...MOTION.logo[name].$value.pivots };
  const lens = Object.fromEntries(markParts({ arcs: 2 }).filter((p) => p.stroke).map((p) => [p.name, p.len]));
  const parts = markElements({ ids: true, arcs: 2 }).split("/>").filter(Boolean).map((p) => `${p.replace(' pathLength="1"', "")}/>`);
  const tf = (part) => {
    const k = tracks[part] ?? [];
    const [px, py] = pivots[part];
    const s = v(k, "scale", 1);
    const o = v(k, "opacity", part === "arc2" ? (name === "talking" || name === "searching" ? 1 : 0) : 1);
    const draw = v(k, "draw", 1);
    return { t: `translate(${v(k, "tx", 0)} ${v(k, "ty", 0)}) rotate(${v(k, "rot", 0)} ${px} ${py}) translate(${px} ${py}) scale(${s}) translate(${-px} ${-py})`, o, draw };
  };
  const m = tf("mark");
  const inner = parts
    .map((p) => {
      const name2 = p.match(/id="(\w+)"/)[1];
      const q = tf(name2);
      const L = lens[name2];
      const dash = L ? ` stroke-dasharray="${L} ${L}" stroke-dashoffset="${(L * (1 - q.draw)).toFixed(3)}"` : "";
      return `<g transform="${q.t}" opacity="${q.o}">${p.replace("/>", `${dash}/>`)}</g>`;
    })
    .join("");
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 120"><rect width="120" height="120" rx="20" fill="${C.graphite}"/>${place(`<g transform="${m.t}">${inner}</g>`, 60, 60, 40)}</svg>`;
}

export async function writeMarketing() {
  out("docs/store/feature-graphic.svg", featureGraphic("en"));
  out("docs/store/feature-graphic.png", png(featureGraphic("en"), 1024));
  out("docs/store/feature-graphic-ro.png", png(featureGraphic("ro"), 1024));
  out("apps/web/public/og.png", png(ogImage("Talk anywhere.", "No signal needed."), 1200));
  out("apps/web/public/og-ro.png", png(ogImage("Vorbește oriunde.", "Fără semnal."), 1200));
  out("brand/templates/social-avatar.png", png(tile(800, { rx: null }), 800));
  out("brand/templates/social-banner-x.png", png(banner(1500, 500), 1500));
  out("brand/templates/social-banner-linkedin.png", png(banner(1584, 396), 1584));
  out("brand/templates/readme-header.svg", readmeHeader());
  out("docs/brand-header.png", png(readmeHeader(), 1280));
  for (const l of ["en", "ro"]) {
    const card = await inviteCard(l);
    out(`brand/templates/invite-card-a6-${l}.svg`, card);
    out(`brand/templates/invite-card-a6-${l}.png`, png(card.replace('width="105mm" height="148mm"', 'width="1240" height="1748"'), 1240));
  }
  for (const n of Object.keys(STICKERS)) out(`brand/stickers/${n}.png`, png(sticker(n), 512));
  out("brand/concepts/brand-sheet.png", png(sheet(), 1360));
}
