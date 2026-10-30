// Tokens: palette.mjs parameters -> DTCG 2025.10 files in brand/tokens (+ contrast pairs),
// then tokens -> platform outputs consumed by every surface.
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { build, semantic, contrastPairs, SEMANTIC, resolveRef } from "./palette.mjs";
import { oklchToHex } from "./color.mjs";
import { out, ROOT, GENERATED } from "./io.mjs";
import { MOTION } from "./motion-spec.mjs";

const RAMPS = ["neutral", "amber", "receive", "danger", "success", "emergency"];
const JOBS = ["app background", "subtle background", "component bg", "component bg hover", "component bg active", "subtle border", "border", "strong border / focus", "solid", "solid hover", "low-contrast text", "high-contrast text"];
const json = (o) => `${JSON.stringify(o, null, 2)}\n`;
const colorValue = ([L, C, H]) => ({ colorSpace: "oklch", components: [L, C, H], hex: oklchToHex(L, C, H).hex });

export function writeTokens() {
  const p = build("signal");
  const prim = { $description: "Layer 1 - raw 12-step ramps (Radix step jobs). Components never read these; semantic tokens reference them. Source: brand/scripts/palette.mjs (direction 'signal').", color: { $type: "color" } };
  for (const theme of ["dark", "light"]) {
    prim.color[theme] = {};
    for (const ramp of RAMPS) {
      prim.color[theme][ramp] = {};
      p[theme][ramp].forEach((c, i) => (prim.color[theme][ramp][i + 1] = { $value: colorValue(c), $description: JOBS[i] }));
    }
  }
  out("brand/tokens/primitives.tokens.json", json(prim));

  const sem = { $description: "Layer 2 - references only. Themes remap ONLY this layer. Dark is the primary theme." };
  for (const theme of ["dark", "light"]) {
    sem[theme] = {};
    for (const [name, entry] of Object.entries(SEMANTIC)) {
      const [group, key] = name.split(".");
      const r = resolveRef(entry, theme);
      sem[theme][group] ??= { $type: "color" };
      sem[theme][group][key] = { $value: r.literal ? colorValue(r.literal) : `{color.${r.theme}.${r.ramp}.${r.step}}` };
    }
  }
  out("brand/tokens/semantic.tokens.json", json(sem));

  const dim = (value, unit = "px") => ({ value, unit });
  const typo = {
    $description: "Type tokens. Families pass brand/scripts/check-glyphs.py (Romanian ă â î ș ț + cedilla legacy) before shipping.",
    font: {
      family: {
        $type: "fontFamily",
        ui: { $value: ["Manrope", "ui-sans-serif", "system-ui", "Segoe UI", "Roboto", "sans-serif"] },
        code: { $value: ["Geist Mono", "ui-monospace", "SFMono-Regular", "Cascadia Code", "monospace"] },
      },
      weight: { $type: "fontWeight", regular: { $value: 400 }, medium: { $value: 500 }, semibold: { $value: 600 }, bold: { $value: 700 }, extrabold: { $value: 800 } },
      size: { $type: "dimension", caption: { $value: dim(12) }, body: { $value: dim(16) }, title: { $value: dim(20) }, headline: { $value: dim(28) }, display: { $value: dim(40) } },
      tracking: { $type: "dimension", display: { $value: dim(-1.5) }, headline: { $value: dim(-0.5) }, body: { $value: dim(0) } },
    },
    radius: { $type: "dimension", xl: { $value: dim(22) }, "2xl": { $value: dim(28) }, pill: { $value: dim(999) } },
  };
  out("brand/tokens/typography.tokens.json", json(typo));
  out("brand/tokens/motion.tokens.json", json(MOTION));

  const pairs = contrastPairs(semantic(p));
  pairs.$description = "Every shipped fg/bg pairing per theme. Gate: node brand/scripts/contrast-gate.mjs brand/contrast-pairs.json (WCAG 2.2 AA fails exit 1; APCA advisory).";
  out("brand/contrast-pairs.json", json(pairs));
}

// ---------- tokens -> platform outputs ----------
function load() {
  const read = (f) => JSON.parse(readFileSync(resolve(ROOT, "brand/tokens", f), "utf8"));
  const prim = read("primitives.tokens.json");
  const sem = read("semantic.tokens.json");
  const deref = (v) => {
    if (typeof v !== "string") return v;
    const [, theme, ramp, step] = v.match(/^\{color\.(\w+)\.(\w+)\.(\d+)\}$/);
    return prim.color[theme][ramp][step].$value;
  };
  const flat = (theme) => {
    const o = {};
    for (const [g, grp] of Object.entries(sem[theme])) for (const [k, t] of Object.entries(grp)) if (k !== "$type") o[`${g}.${k}`] = deref(t.$value);
    return o;
  };
  return { prim, dark: flat("dark"), light: flat("light"), typo: read("typography.tokens.json"), motion: read("motion.tokens.json") };
}

const camel = (s) => s.replace(/[.-](\w)/g, (_, c) => c.toUpperCase());
const kebab = (s) => s.replace(/\./g, "-");
const oklchCss = (v) => `oklch(${v.components[0]} ${v.components[1]} ${v.components[2]})`;
const bez = (a) => `cubic-bezier(${a.join(", ")})`;

export function writeOutputs() {
  const t = load();
  // CSS (Tailwind v4 @theme). Legacy names kept as aliases so every existing class keeps working.
  const legacy = {
    graphite: "bg.canvas", surface: "bg.surface", elevated: "bg.elevated", outline: "border.default", ink: "fg.default", muted: "fg.muted",
    amber: "accent.solid", "amber-deep": "accent.pressed", teal: "receive.solid", danger: "danger.solid", success: "success.solid", emergency: "emergency.solid",
  };
  let css = `/* ${GENERATED("tokens.mjs")} */\n@theme {\n`;
  for (const [k, v] of Object.entries(t.dark)) css += `  --color-${kebab(k)}: ${v.hex};\n`;
  for (const [alias, k] of Object.entries(legacy)) css += `  --color-${alias}: var(--color-${kebab(k)});\n`;
  const mo = t.motion;
  for (const [k, v] of Object.entries(mo.duration)) if (!k.startsWith("$")) css += `  --duration-${k}: ${v.$value.value}ms;\n`;
  for (const [k, v] of Object.entries(mo.easing)) if (!k.startsWith("$")) css += `  --ease-${k}: ${bez(v.$value)};\n`;
  css += `}\n\n@supports (color: oklch(0 0 0)) {\n  :root {\n`;
  for (const [k, v] of Object.entries(t.dark)) css += `    --color-${kebab(k)}: ${oklchCss(v)};\n`;
  css += `  }\n}\n\n/* Light theme: opt in with data-theme="light" on <html>. */\n[data-theme="light"] {\n  color-scheme: light;\n`;
  for (const [k, v] of Object.entries(t.light)) css += `  --color-${kebab(k)}: ${v.hex};\n`;
  css += `}\n`;
  out("packages/app-ui/src/brand-tokens.css", css);

  // TypeScript
  let ts = `// ${GENERATED("tokens.mjs")}\nexport const brand = {\n  dark: {\n`;
  for (const [k, v] of Object.entries(t.dark)) ts += `    ${JSON.stringify(k)}: ${JSON.stringify(v.hex)},\n`;
  ts += `  },\n  light: {\n`;
  for (const [k, v] of Object.entries(t.light)) ts += `    ${JSON.stringify(k)}: ${JSON.stringify(v.hex)},\n`;
  ts += `  },\n} as const;\nexport type BrandColor = keyof typeof brand.dark;\n`;
  out("packages/app-ui/src/brand-tokens.ts", ts);

  // Kotlin (android/client is shared by phone, Wear and TV)
  const argb = (v) => `0xFF${v.hex.slice(1).toUpperCase()}`;
  let kt = `// ${GENERATED("tokens.mjs")}\npackage ro.titi.brand\n\nimport androidx.compose.ui.graphics.Color\n\n`;
  for (const theme of ["dark", "light"]) {
    kt += `object Brand${theme === "dark" ? "Dark" : "Light"} {\n`;
    for (const [k, v] of Object.entries(t[theme])) kt += `    val ${camel(k)} = Color(${argb(v)})\n`;
    kt += `}\n\n`;
  }
  kt += `object BrandMotion {\n`;
  for (const [k, v] of Object.entries(mo.duration)) if (!k.startsWith("$")) kt += `    const val ${camel(`duration.${k}`)}Ms = ${v.$value.value}\n`;
  kt += `    const val springStiffness = ${mo.spring.talk.$value.stiffness}f\n    const val springDamping = ${mo.spring.talk.$value.damping}f\n}\n`;
  out("android/client/src/main/kotlin/ro/titi/brand/BrandTokens.kt", kt);

  // Swift
  const rgb = (v) => {
    const n = (i) => (parseInt(v.hex.slice(1 + i * 2, 3 + i * 2), 16) / 255).toFixed(4);
    return `Color(.sRGB, red: ${n(0)}, green: ${n(1)}, blue: ${n(2)}, opacity: 1)`;
  };
  let sw = `// ${GENERATED("tokens.mjs")}\nimport SwiftUI\n\n`;
  for (const theme of ["dark", "light"]) {
    sw += `enum Brand${theme === "dark" ? "Dark" : "Light"} {\n`;
    for (const [k, v] of Object.entries(t[theme])) sw += `    static let ${camel(k)} = ${rgb(v)}\n`;
    sw += `}\n\n`;
  }
  out("ios/Titi/BrandTokens.swift", sw);

  // Android resource colours (launcher bg, splash, accent)
  const xml = `<?xml version="1.0" encoding="utf-8"?>\n<!-- ${GENERATED("tokens.mjs")} -->\n<resources>\n    <color name="graphite">#FF${t.dark["bg.canvas"].hex.slice(1).toUpperCase()}</color>\n    <color name="amber">#FF${t.dark["accent.solid"].hex.slice(1).toUpperCase()}</color>\n    <color name="ic_launcher_background">#FF${t.dark["bg.canvas"].hex.slice(1).toUpperCase()}</color>\n</resources>\n`;
  for (const m of ["app", "wear", "tv"]) out(`android/${m}/src/main/res/values/colors.xml`, xml);
  return t;
}
