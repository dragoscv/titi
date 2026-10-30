// OKLCH <-> sRGB helpers shared by the brand scripts (no deps).
const enc = (v) => (v <= 0.0031308 ? 12.92 * v : 1.055 * v ** (1 / 2.4) - 0.055);
const dec = (v) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);

function labToLin(L, a, b) {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
}
const inGamut = (rgb) => rgb.every((v) => v >= -1e-6 && v <= 1 + 1e-6);

/** OKLCH -> { hex, clipped } ; out-of-gamut chroma is reduced (L and H kept). */
export function oklchToHex(L, C, H) {
  const lin = (c) => labToLin(L, c * Math.cos((H * Math.PI) / 180), c * Math.sin((H * Math.PI) / 180));
  let rgb = lin(C);
  let clipped = false;
  if (!inGamut(rgb)) {
    clipped = true;
    let lo = 0;
    let hi = C;
    for (let i = 0; i < 40; i++) {
      const mid = (lo + hi) / 2;
      if (inGamut(lin(mid))) lo = mid;
      else hi = mid;
    }
    rgb = lin(lo);
  }
  const hex = rgb
    .map((v) => Math.round(Math.min(1, Math.max(0, enc(Math.min(1, Math.max(0, v))))) * 255).toString(16).padStart(2, "0"))
    .join("");
  return { hex: `#${hex}`, clipped };
}

/** sRGB hex -> [L, C, H] */
export function hexToOklch(hex) {
  const [r, g, b] = [1, 3, 5].map((i) => dec(parseInt(hex.slice(i, i + 2), 16) / 255));
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  const L = 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s;
  const a = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s;
  const bb = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
  const C = Math.hypot(a, bb);
  const H = ((Math.atan2(bb, a) * 180) / Math.PI + 360) % 360;
  return [+L.toFixed(4), +C.toFixed(4), +H.toFixed(2)];
}

export const css = ([L, C, H]) => `oklch(${L} ${C} ${H})`;
