// Titi "Chirp" mark — the ONE geometry source for every export, animation and sticker.
// 48-unit keyline grid. Body circle + speech-bubble tail + beak + eye + one signal arc
// concentric with the body. Optical sizes: master (>=64 px), small (24-48 px), micro (16-20 px).
import { readFileSync } from "node:fs";
import { build, semantic } from "./palette.mjs";
import { oklchToHex } from "./color.mjs";

export const pal = build("signal");
export const sem = semantic(pal);
export const hex = (c) => oklchToHex(...c).hex;

/** Brand colours used by the mark (resolved from tokens, never typed by hand). */
export const C = {
  graphite: hex(pal.dark.neutral[0]),
  surface: hex(pal.dark.neutral[1]),
  ink: hex(pal.dark.neutral[11]),
  muted: hex(pal.dark.neutral[10]),
  amber: hex(pal.dark.amber[8]),
  amberDeep: "#D98E00",
  teal: hex(pal.dark.receive[8]),
  danger: hex(pal.dark.danger[8]),
  emergency: hex(pal.dark.emergency[8]),
  light: hex(pal.light.neutral[0]),
  lightInk: hex(pal.light.neutral[11]),
};

export const BODY = { cx: 22, cy: 26, r: 11 };
export const CENTER = { x: 22.5, y: 25 }; // optical centre used for placement
export const RADIUS = 18.9; // farthest ink from CENTER (arc + stroke), for safe-zone fitting
export const VARIANTS = {
  master: { eye: 1.9, arc: 2.6, arcR: 16.5, a0: 205, a1: 255 },
  small: { eye: 2.3, arc: 3.0, arcR: 16.8, a0: 203, a1: 257 },
  micro: { eye: 2.9, arc: 3.6, arcR: 17.2, a0: 200, a1: 260 },
};

const f = (n) => +n.toFixed(3);
export const circle = (cx, cy, r) => `M${f(cx - r)},${f(cy)}a${r},${r} 0 1,0 ${f(2 * r)},0a${r},${r} 0 1,0 ${f(-2 * r)},0Z`;
export const arcPath = (r, a0, a1, cx = BODY.cx, cy = BODY.cy) => {
  const p = (a) => `${f(cx + r * Math.cos((a * Math.PI) / 180))},${f(cy + r * Math.sin((a * Math.PI) / 180))}`;
  return `M${p(a0)}A${r},${r} 0 0,1 ${p(a1)}`;
};

export const PATHS = {
  body: circle(BODY.cx, BODY.cy, BODY.r),
  tail: "M12.03,30.65L17.35,35.97L11,39.5Z",
  beak: "M32,22.6L38.2,26L32,29.4Z",
};
export const EYE = { cx: 26.2, cy: 22.4 };

/**
 * Structured parts in grid units: [{ name, d, fill } | { name, d, stroke, width }].
 * colors: {body, beak, eye, arc}; mono=true cuts the eye as a hole (one-colour / themed icons).
 * arcs: number of signal arcs (1 = brand; 2 = talking; 0 = muted, which adds a slash).
 */
export function markParts({ variant = "master", colors = {}, mono = false, arcs = 1, muted = false } = {}) {
  const v = VARIANTS[variant];
  const col = { body: C.amber, beak: C.amberDeep, eye: C.graphite, arc: C.teal, ...colors };
  const eyeHole = circle(EYE.cx, EYE.cy, v.eye);
  const parts = [];
  parts.push(mono ? { name: "body", d: PATHS.body + eyeHole, fill: col.body, evenOdd: true } : { name: "body", d: PATHS.body, fill: col.body });
  parts.push({ name: "tail", d: PATHS.tail, fill: col.body });
  parts.push({ name: "beak", d: PATHS.beak, fill: mono ? col.body : col.beak });
  if (!mono) parts.push({ name: "eye", d: eyeHole, fill: col.eye });
  for (let i = 0; i < arcs; i++) {
    const r = v.arcR + i * (v.arc * 2.2);
    const a0 = v.a0 + i * 6;
    const a1 = v.a1 - i * 6;
    parts.push({ name: i ? `arc${i + 1}` : "arc", d: arcPath(r, a0, a1), stroke: col.arc, width: v.arc, len: f((r * (a1 - a0) * Math.PI) / 180) });
  }
  if (muted) {
    // slash across the arc's midpoint, perpendicular to it
    const m = (((v.a0 + v.a1) / 2) * Math.PI) / 180;
    const t = m + Math.PI / 2;
    const mx = BODY.cx + v.arcR * Math.cos(m);
    const my = BODY.cy + v.arcR * Math.sin(m);
    const L = 6.5;
    parts.push({ name: "slash", d: `M${f(mx - L * Math.cos(t))},${f(my - L * Math.sin(t))}L${f(mx + L * Math.cos(t))},${f(my + L * Math.sin(t))}`, stroke: col.slash ?? C.danger, width: v.arc });
  }
  return parts;
}

/** SVG rendering of markParts. ids=true adds ids + pathLength=1 on strokes (animation targets). */
export function markElements(opts = {}) {
  return markParts(opts)
    .map((p) => {
      const id = opts.ids ? ` id="${p.name}"` : "";
      if (p.stroke) return `<path${id} d="${p.d}" stroke="${p.stroke}" stroke-width="${p.width}" stroke-linecap="round" fill="none"${opts.ids ? ' pathLength="1"' : ""}/>`;
      return `<path${id}${p.evenOdd ? ' fill-rule="evenodd"' : ""} d="${p.d}" fill="${p.fill}"/>`;
    })
    .join("");
}

/** Placement transform numbers for drawing the grid at (cx, cy) with ink radius r. */
export const placement = (cx, cy, r) => {
  const s = r / RADIUS;
  return { tx: f(cx - CENTER.x * s), ty: f(cy - CENTER.y * s), s: f(s) };
};

/** Group that maps the grid into a canvas: mark centred at (cx, cy) with ink radius `r`. */
export const place = (inner, cx, cy, r) => {
  const p = placement(cx, cy, r);
  return `<g transform="translate(${p.tx} ${p.ty}) scale(${p.s})">${inner}</g>`;
};

/** Square app tile (graphite rounded square) — web, desktop, iOS, Tizen, stores. */
export function tile(size, { variant, rx = 0.225, bg = C.graphite, fill = 0.36, colors, mono, arcs, muted } = {}) {
  const v = variant ?? (size <= 20 ? "micro" : size <= 48 ? "small" : "master");
  const r = size * (v === "micro" ? 0.44 : v === "small" ? 0.4 : fill);
  const rect = rx === null ? "" : `<rect width="${size}" height="${size}" rx="${f(size * rx)}" fill="${bg}"/>`;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${size} ${size}" width="${size}" height="${size}">${rect}${place(markElements({ variant: v, colors, mono, arcs, muted }), size / 2, size / 2 + size * 0.01, r)}</svg>`;
}

// ---------- wordmark: lettered from Manrope ExtraBold (OFL 1.1) ----------
const glyphs = JSON.parse(readFileSync(new URL("../logo/src/manrope-800.json", import.meta.url), "utf8"));
// Lettering changes (registrable, not typeable): t crossbar cut short on the left (flag t),
// tittles redrawn as round amber "signal dots", one larger than the stem, tracking -36 units.
const LETTER = {
  t: { d: glyphs.glyphs.t.d.replace("M20 870V1080H788V870Z", "M118 870V1080H788V870Z"), adv: glyphs.glyphs.t.adv },
  i: { d: "M160 0V1080H432V0Z", adv: glyphs.glyphs.i.adv, dot: { cx: 296, cy: 1352, r: 158 } },
};
const TRACK = -36;

/** Wordmark paths (font units, y-up flipped). Returns {svg inner, width, height}. */
export function wordmark({ ink = C.ink, dot = C.amber } = {}) {
  let x = 0;
  let letters = "";
  let dots = "";
  const parts = [];
  for (const ch of "titi") {
    const g = LETTER[ch];
    letters += `<path transform="translate(${x} 0)" d="${g.d}"/>`;
    parts.push(["letter", flipPath(g.d, x), ink]);
    if (g.dot) {
      const c = circle(x + g.dot.cx, -g.dot.cy, g.dot.r);
      dots += `<path d="${c}"/>`;
      parts.push(["dot", c, dot]);
    }
    x += g.adv + TRACK;
  }
  const width = x - TRACK - 40;
  const top = 1352 + 158;
  return {
    inner: `<g fill="${ink}" transform="scale(1 -1)">${letters}</g><g fill="${dot}">${dots}</g>`,
    parts,
    width,
    top,
    bottom: 24,
  };
}

/** Glyph path (font units, y-up, absolute M/L/H/V/Q/Z only) -> y-down, shifted by dx. */
function flipPath(d, dx) {
  return d.replace(/([MLHVQZ])([^MLHVQZ]*)/g, (_, cmd, args) => {
    const n = args.trim() ? args.trim().split(/[\s,]+/).map(Number) : [];
    const o = cmd === "H" ? n.map((v) => v + dx) : cmd === "V" ? n.map((v) => -v) : n.map((v, i) => (i % 2 ? -v : v + dx));
    return cmd + o.map((v) => +v.toFixed(2)).join(" ");
  });
}

export function wordmarkSvg(opts = {}, pad = 40) {
  const w = wordmark(opts);
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${-pad} ${-w.top - pad} ${w.width + 2 * pad} ${w.top + w.bottom + 2 * pad}">${w.inner}</svg>`;
}

/** Horizontal lockup: symbol height = 1.25 x-height-to-tittle height, gap = 0.5 symbol. */
export function lockup({ ink = C.ink, dot = C.amber, colors, layout = "horizontal", bg } = {}) {
  const w = wordmark({ ink, dot });
  const h = w.top + w.bottom;
  const r = h * 0.5;
  if (layout === "stacked") {
    const W = Math.max(w.width, r * 2.2) + 160;
    const markCy = r + 80;
    const textTop = markCy + r + 120;
    const H = textTop + h + 80;
    return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${f(W)} ${f(H)}">${bg ? `<rect width="100%" height="100%" fill="${bg}"/>` : ""}${place(markElements({ colors }), W / 2, markCy, r)}<g transform="translate(${f((W - w.width) / 2)} ${f(textTop + w.top)})">${w.inner}</g></svg>`;
  }
  const gap = r * 0.4;
  const W = r * 2 + gap + w.width + 120;
  const H = Math.max(h, 2 * r) + 120;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${f(W)} ${f(H)}">${bg ? `<rect width="100%" height="100%" fill="${bg}"/>` : ""}${place(markElements({ colors }), 60 + r, H / 2, r)}<g transform="translate(${f(60 + 2 * r + gap)} ${f((H - h) / 2 + w.top)})">${w.inner}</g></svg>`;
}
