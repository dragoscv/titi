// Logo motion: MOTION.logo (motion-spec.mjs) -> CSS keyframes (web/desktop/TV + animated SVG)
// and an Android AnimatedVectorDrawable (splash intro). Static mark == final frame of `intro`.
import { MOTION, PIVOTS } from "./motion-spec.mjs";
import { markParts, CENTER, RADIUS } from "./mark.mjs";
import { out, GENERATED } from "./io.mjs";

const EASE = Object.fromEntries(Object.entries(MOTION.easing).filter(([k]) => !k.startsWith("$")).map(([k, v]) => [k, `cubic-bezier(${v.$value.join(", ")})`]));
// Animatable viewBox in grid units, centred on the optical centre (so CSS transform-origin is in grid units).
// R2 = reach of the second (talking) arc from the optical centre, so live states never clip.
const R2 = 24.4;
export const VIEWBOX = [CENTER.x - R2, CENTER.y - R2, 2 * R2, 2 * R2].map((n) => +n.toFixed(2));

const cssProp = (prop, v) =>
  ({ scale: `scale: ${v}`, tx: `translate: ${v}px 0`, ty: `translate: 0 ${v}px`, rot: `rotate: ${v}deg`, opacity: `opacity: ${v}`, draw: `stroke-dashoffset: ${+(1 - v).toFixed(3)}` })[prop];

/** CSS for every logo animation, scoped by [data-mark-state] on the svg. */
export function logoCss() {
  let css = `/* ${GENERATED("motion.mjs")} */\n`;
  css += `.titi-mark [data-part] { transform-box: view-box; }\n`;
  for (const [part, [x, y]] of Object.entries(PIVOTS)) css += `.titi-mark [data-part="${part}"] { transform-origin: ${x}px ${y}px; }\n`;
  css += `.titi-mark path[stroke] { stroke-dasharray: 1; stroke-dashoffset: 0; }\n`;
  css += `.titi-mark [data-part="arc2"] { opacity: 0; }\n`;
  for (const [name, tok] of Object.entries(MOTION.logo)) {
    const { duration, ease, loop, tracks, pivots = {} } = tok.$value;
    for (const [part, keys] of Object.entries(tracks)) {
      const kf = `titi-${name}-${part}`;
      css += `@keyframes ${kf} {\n`;
      const byT = new Map();
      for (const [t, prop, v] of keys) byT.set(t, [...(byT.get(t) ?? []), cssProp(prop, v)]);
      // hold each property's last value to 100 % (otherwise CSS interpolates back to the base value)
      const last = new Map();
      for (const [t, prop, v] of keys) if (!last.has(prop) || last.get(prop)[0] <= t) last.set(prop, [t, v]);
      for (const [prop, [t, v]] of last) if (t < 1) byT.set(1, [...(byT.get(1) ?? []), cssProp(prop, v)]);
      for (const [t, decls] of [...byT].sort((a, b) => a[0] - b[0])) css += `  ${+(t * 100).toFixed(1)}% { ${decls.join("; ")}; }\n`;
      css += `}\n`;
      const hold = ["hover", "press", "morph"].includes(name) ? "forwards" : loop ? "none" : "both";
      const origin = pivots[part] ? ` transform-origin: ${pivots[part][0]}px ${pivots[part][1]}px;` : "";
      css += `.titi-mark[data-mark-state="${name}"] [data-part="${part}"] {${origin} animation: ${kf} ${duration}ms ${EASE[ease]} ${loop ? "infinite" : "1"} ${hold}; }\n`;
    }
  }
  css += `@media (prefers-reduced-motion: reduce) {\n  .titi-mark [data-part] { animation: none !important; }\n  .titi-mark[data-mark-state="talking"] [data-part="arc2"] { opacity: 1; }\n}\n`;
  return css;
}

/** Animatable SVG body: <g data-part="mark"> wrapping one path per part (pathLength=1 on strokes). */
export function markBody() {
  const parts = markParts({ arcs: 2 });
  return `<g data-part="mark">${parts.map((p) => (p.stroke ? `<path data-part="${p.name}" d="${p.d}" stroke="${p.stroke}" stroke-width="${p.width}" stroke-linecap="round" fill="none" pathLength="1"/>` : `<path data-part="${p.name}" d="${p.d}" fill="${p.fill}"/>`)).join("")}</g>`;
}

/** Self-contained animated SVG (README/docs/email-safe; static first frame is the final mark under reduced motion). */
export function animatedSvg(state = "intro", { loopIntro = false } = {}) {
  let css = logoCss().replace(/^\/\*.*\*\/\n/, "");
  if (loopIntro) css = css.replace(/(titi-intro-\w+ 900ms [^;]*?) 1 both/g, "$1 infinite both");
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${VIEWBOX.join(" ")}" class="titi-mark" data-mark-state="${state}" role="img" aria-label="Titi"><style>${css}</style>${markBody()}</svg>`;
}

/** Geometry for the React <BrandMark>, so the component never hard-codes paths. */
export function markTs() {
  const parts = markParts({ arcs: 2 }).map(({ name, d, fill, stroke, width }) => ({ name, d, ...(fill ? { fill } : { stroke, width }) }));
  const muted = markParts({ muted: true }).find((p) => p.name === "slash");
  return `// ${GENERATED("motion.mjs")}\nexport const MARK_VIEWBOX = ${JSON.stringify(VIEWBOX.join(" "))};\nexport const MARK_PARTS = ${JSON.stringify(parts, null, 2)} as const;\nexport const MARK_SLASH = ${JSON.stringify({ d: muted.d, width: muted.width })} as const;\nexport type MarkState = ${Object.keys(MOTION.logo).map((k) => JSON.stringify(k)).join(" | ")} | "idle" | "muted";\n`;
}

// ---------- Android AVD (splash intro) ----------
const AV_PROP = { scale: ["scaleX", "scaleY"], tx: ["translateX"], ty: ["translateY"], rot: ["rotation"] };
const AV_INTERP = { chirp: "@android:anim/overshoot_interpolator", move: "@android:interpolator/fast_out_slow_in", enter: "@android:interpolator/linear_out_slow_in" };

export function splashAvd({ cx = 54, cy = 54, r = 30, size = 108 } = {}) {
  const intro = MOTION.logo.intro.$value;
  const s = r / RADIUS;
  const tx = +(cx - CENTER.x * s).toFixed(3);
  const ty = +(cy - CENTER.y * s).toFixed(3);
  const parts = markParts();
  const color = (h) => `#FF${h.slice(1).toUpperCase()}`;
  let vec = `<vector android:width="${size}dp" android:height="${size}dp" android:viewportWidth="${size}" android:viewportHeight="${size}">\n`;
  vec += `            <group android:translateX="${tx}" android:translateY="${ty}" android:scaleX="${+s.toFixed(4)}" android:scaleY="${+s.toFixed(4)}">\n`;
  for (const p of parts) {
    const [px, py] = PIVOTS[p.name] ?? PIVOTS.mark;
    vec += `                <group android:name="g_${p.name}" android:pivotX="${px}" android:pivotY="${py}">\n`;
    vec += p.stroke
      ? `                    <path android:name="p_${p.name}" android:pathData="${p.d}" android:strokeColor="${color(p.stroke)}" android:strokeWidth="${p.width}" android:strokeLineCap="round" />\n`
      : `                    <path android:name="p_${p.name}" android:pathData="${p.d}" android:fillColor="${color(p.fill)}" />\n`;
    vec += `                </group>\n`;
  }
  vec += `            </group>\n        </vector>`;
  let targets = "";
  const isStroke = Object.fromEntries(parts.map((p) => [p.name, !!p.stroke]));
  for (const [part, keys] of Object.entries(intro.tracks)) {
    const props = {};
    for (const [t, prop, v] of keys) (props[prop] ??= []).push([t, v]);
    const byTarget = { [`g_${part}`]: [], [`p_${part}`]: [] };
    for (const [prop, kfs] of Object.entries(props)) {
      const onPath = prop === "draw" || prop === "opacity";
      const names = prop === "draw" ? ["trimPathEnd"] : prop === "opacity" ? [isStroke[part] ? "strokeAlpha" : "fillAlpha"] : AV_PROP[prop];
      const full = kfs[kfs.length - 1][0] < 1 ? [...kfs, [1, kfs[kfs.length - 1][1]]] : kfs;
      for (const n of names) {
        byTarget[onPath ? `p_${part}` : `g_${part}`].push(`                    <propertyValuesHolder android:propertyName="${n}">\n${full.map(([t, v], i) => `                        <keyframe android:fraction="${t}" android:value="${v}"${i ? ` android:interpolator="${AV_INTERP[intro.ease]}"` : ""} />`).join("\n")}\n                    </propertyValuesHolder>`);
      }
    }
    for (const [target, holders] of Object.entries(byTarget)) {
      if (!holders.length) continue;
      targets += `    <target android:name="${target}">\n        <aapt:attr name="android:animation">\n            <objectAnimator android:duration="${intro.duration}" android:valueType="floatType">\n${holders.join("\n")}\n            </objectAnimator>\n        </aapt:attr>\n    </target>\n`;
    }
  }
  return `<?xml version="1.0" encoding="utf-8"?>\n<!-- ${GENERATED("motion.mjs")} Splash intro (${intro.duration} ms, Android 12+ windowSplashScreenAnimatedIcon). -->\n<animated-vector xmlns:android="http://schemas.android.com/apk/res/android"\n    xmlns:aapt="http://schemas.android.com/aapt">\n    <aapt:attr name="android:drawable">\n        ${vec}\n    </aapt:attr>\n${targets}</animated-vector>\n`;
}

export function writeMotion() {
  out("packages/app-ui/src/brand-mark.css", logoCss());
  out("packages/app-ui/src/brand-mark.ts", markTs());
  out("brand/logo/animated/intro.svg", animatedSvg("intro"));
  out("brand/logo/animated/intro-loop.svg", animatedSvg("intro", { loopIntro: true }));
  for (const s of ["searching", "talking"]) out(`brand/logo/animated/${s}.svg`, animatedSvg(s));
  const avd = splashAvd();
  for (const m of ["app", "wear", "tv"]) out(`android/${m}/src/main/res/drawable/splash_intro.xml`, avd);
}
