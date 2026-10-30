// Palette directions for brand v1. Each direction = hues/chroma per role; ramps are derived
// with fixed lightness curves (Radix step jobs, dark theme first).
// Ramps: 12 steps. 1-2 bg, 3-5 component bg, 6-8 borders, 9 solid, 10 solid hover, 11 low-contrast text, 12 high-contrast text.

const DARK_N = [0.172, 0.2125, 0.251, 0.28, 0.3041, 0.35, 0.42, 0.5, 0.6, 0.66, 0.78, 0.9665];
const LIGHT_N = [0.99, 0.975, 0.95, 0.925, 0.9, 0.87, 0.8, 0.62, 0.56, 0.52, 0.46, 0.24];
// accent curves: [L, chroma factor]
const DARK_A = [[0.19, 0.12], [0.22, 0.18], [0.27, 0.3], [0.31, 0.4], [0.35, 0.48], [0.42, 0.55], [0.5, 0.65], [0.6, 0.8], [null, 1], [null, 1], [0.84, 0.8], [0.94, 0.35]];
const LIGHT_A = [[0.99, 0.06], [0.975, 0.12], [0.95, 0.22], [0.92, 0.32], [0.89, 0.42], [0.85, 0.5], [0.79, 0.6], [0.72, 0.75], [null, 1], [null, 1], [0.5, 0.75], [0.3, 0.45]];

function ramp(curve, [L9, C9, H], hover) {
  return curve.map(([L, f], i) => {
    if (i === 8) return [L9, C9, H];
    if (i === 9) return [+(L9 + hover).toFixed(4), C9, H];
    return [L, +(C9 * f).toFixed(4), H];
  });
}
const neutral = (curve, C, H) => curve.map((L, i) => [L, +(i === 11 ? C * 0.4 : C * (i < 6 ? 1 : 1.3)).toFixed(4), H]);

export const directions = {
  signal: {
    label: "Signal Amber, refined",
    neutral: { C: 0.012, H: 258 },
    amber: [0.8131, 0.164, 75.04],
    receive: [0.7845, 0.1325, 181.91],
    danger: [0.65, 0.2, 22],
    success: [0.75, 0.17, 152],
    emergency: [0.6497, 0.2383, 18],
  },
  warm: {
    label: "Warm graphite + coral receive",
    neutral: { C: 0.01, H: 60 },
    amber: [0.8131, 0.165, 75],
    receive: [0.72, 0.15, 35],
    danger: [0.62, 0.21, 12],
    success: [0.75, 0.15, 145],
    emergency: [0.62, 0.24, 5],
  },
  night: {
    label: "Night radio: ink blue + amber + sky",
    neutral: { C: 0.03, H: 265 },
    amber: [0.8131, 0.165, 75],
    receive: [0.76, 0.12, 230],
    danger: [0.65, 0.2, 22],
    success: [0.75, 0.17, 152],
    emergency: [0.6497, 0.2383, 18],
  },
};

export function build(dir) {
  const d = directions[dir];
  const out = { dark: {}, light: {} };
  out.dark.neutral = neutral(DARK_N, d.neutral.C, d.neutral.H);
  out.light.neutral = neutral(LIGHT_N, d.neutral.C * 0.8, d.neutral.H);
  for (const role of ["amber", "receive", "danger", "success", "emergency"]) {
    out.dark[role] = ramp(DARK_A, d[role], 0.035);
    out.light[role] = ramp(LIGHT_A, d[role], -0.04);
  }
  return out;
}

/**
 * Semantic layer as aliases: name -> [ramp, step] per theme (or a literal OKLCH triple).
 * `fg.on-accent` always reads dark neutral 1 (amber/teal solids need dark text in both themes).
 */
export const SEMANTIC = {
  "bg.canvas": ["neutral", 1],
  "bg.surface": ["neutral", 2],
  "bg.elevated": ["neutral", 3],
  "bg.highest": ["neutral", 4],
  "border.subtle": ["neutral", 5],
  "border.default": ["neutral", 6],
  "border.strong": ["neutral", 8],
  "fg.default": ["neutral", 12],
  "fg.muted": ["neutral", 11],
  "accent.solid": ["amber", 9],
  "accent.pressed": ["amber", 10],
  "accent.fg": ["amber", 11],
  "accent.subtle": ["amber", 3],
  "accent.container": ["amber", 4],
  "accent.on-container": ["amber", 12],
  "fg.on-accent": { theme: "dark", ref: ["neutral", 1] },
  "receive.solid": ["receive", 9],
  "receive.fg": ["receive", 11],
  "danger.solid": ["danger", 9],
  "danger.fg": ["danger", 11],
  "success.solid": ["success", 9],
  "success.fg": ["success", 11],
  "emergency.solid": ["emergency", 9],
  "fg.on-emergency": { literal: [1, 0, 0] },
  "focus.ring": { dark: ["amber", 9], light: ["amber", 11] },
};

/** Resolve an alias entry for a theme -> { theme, ramp, step } or { literal }. */
export function resolveRef(entry, theme) {
  if (Array.isArray(entry)) return { theme, ramp: entry[0], step: entry[1] };
  if (entry.literal) return { literal: entry.literal };
  if (entry.ref) return { theme: entry.theme, ramp: entry.ref[0], step: entry.ref[1] };
  const r = entry[theme];
  return { theme, ramp: r[0], step: r[1] };
}

/** Semantic layer values: one function so contrast pairs and outputs never drift. */
export function semantic(p) {
  const s = {};
  for (const theme of ["dark", "light"]) {
    s[theme] = {};
    for (const [name, entry] of Object.entries(SEMANTIC)) {
      const r = resolveRef(entry, theme);
      s[theme][name] = r.literal ?? p[r.theme][r.ramp][r.step - 1];
    }
  }
  return s;
}

export function contrastPairs(sem) {
  const pairs = [];
  const add = (theme, fg, bg, use) => pairs.push({ name: `${theme}: ${fg} on ${bg}`, fg: c(sem[theme][fg]), bg: c(sem[theme][bg]), use });
  const c = ([L, C, H]) => `oklch(${L} ${C} ${H})`;
  for (const t of ["dark", "light"]) {
    for (const bg of ["bg.canvas", "bg.surface", "bg.elevated"]) {
      add(t, "fg.default", bg, "body");
      add(t, "fg.muted", bg, "body");
      add(t, "accent.fg", bg, "body");
      add(t, "receive.fg", bg, "body");
      add(t, "danger.fg", bg, "body");
      add(t, "success.fg", bg, "body");
    }
    add(t, "fg.on-accent", "accent.solid", "body");
    add(t, "fg.on-accent", "accent.pressed", "body");
    add(t, "fg.on-accent", "receive.solid", "body");
    add(t, "fg.on-emergency", "emergency.solid", "large");
    if (t === "dark") add(t, "accent.solid", "bg.canvas", "ui");
    add(t, "focus.ring", "bg.canvas", "ui");
    add(t, "focus.ring", "bg.surface", "ui");
    add(t, "border.strong", "bg.canvas", "ui");
    add(t, "danger.solid", "bg.canvas", "ui");
  }
  return { pairs };
}
