#!/usr/bin/env node
// Validates the App Store Connect listing JSON files against Apple's field limits.
// Usage: node docs/store/ios/check-listing.mjs   (exit 1 on any violation)
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const dir = dirname(fileURLToPath(import.meta.url));
const files = readdirSync(dir).filter((f) => /^listing\..+\.json$/.test(f)).sort();

// Apple counts characters (grapheme-ish); code points are the closest stable measure.
const chars = (s) => [...s].length;
const bytes = (s) => Buffer.byteLength(s, "utf8");
const fold = (s) => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
const words = (s) => fold(s).split(/[^\p{L}\p{N}]+/u).filter(Boolean);

const LIMITS = { name: 30, subtitle: 30, promotionalText: 170, description: 4000, whatsNew: 4000 };
const URLS = {
  supportUrl: "https://titi.dragoscatalin.ro/support",
  marketingUrl: "https://titi.dragoscatalin.ro",
  privacyPolicyUrl: "https://titi.dragoscatalin.ro/privacy",
};
const EN_NAME = "Titi – Offline Walkie Talkie";

let failed = 0;
const fail = (f, msg) => {
  failed++;
  console.log(`  FAIL ${f}: ${msg}`);
};

if (files.length < 2) fail("dir", `expected at least 2 listing files, found ${files.length}`);

for (const f of files) {
  const l = JSON.parse(readFileSync(join(dir, f), "utf8"));
  console.log(`${f} (${l.locale})`);
  for (const [k, max] of Object.entries(LIMITS)) {
    const v = l[k];
    if (typeof v !== "string" || v.trim() === "") {
      fail(f, `${k} missing`);
      continue;
    }
    const n = chars(v);
    console.log(`  ${k.padEnd(16)} ${String(n).padStart(4)} / ${max} chars`);
    if (n > max) fail(f, `${k} is ${n} chars (> ${max})`);
    if (/<[a-z/][^>]*>/i.test(v)) fail(f, `${k} contains HTML`);
    const emoji = v.match(/\p{Extended_Pictographic}/gu) ?? [];
    if (emoji.length > 0) fail(f, `${k} contains emoji (${emoji.join("")})`);
  }

  const kw = l.keywords ?? "";
  const kb = bytes(kw);
  console.log(`  ${"keywords".padEnd(16)} ${String(kb).padStart(4)} / 100 bytes (${chars(kw)} chars)`);
  if (kb > 100) fail(f, `keywords are ${kb} bytes (> 100)`);
  if (/,\s/.test(kw) || /\s,/.test(kw)) fail(f, "keywords have spaces around commas");
  const list = kw.split(",");
  if (list.some((k) => k.trim() === "")) fail(f, "empty keyword");
  const dup = list.filter((k, i) => list.findIndex((x) => fold(x) === fold(k)) !== i);
  if (dup.length) fail(f, `duplicate keywords: ${dup.join(", ")}`);
  const nameWords = new Set(words(l.name));
  const clash = [...new Set(list.flatMap(words))].filter((w) => nameWords.has(w));
  if (clash.length) fail(f, `keywords repeat words from the name: ${clash.join(", ")}`);

  for (const [k, want] of Object.entries(URLS)) if (l[k] !== want) fail(f, `${k} must be ${want}`);
  if (l.locale === "en-US" && l.name !== EN_NAME) fail(f, `en-US name must be exactly "${EN_NAME}"`);
}

// App Review "Notes" field (review-notes.md, between the last START/END markers): 4000 chars.
{
  const t = readFileSync(join(dir, "review-notes.md"), "utf8");
  const a = "<!-- NOTES START -->";
  const b = "<!-- NOTES END -->";
  const s = t.lastIndexOf(a);
  const e = t.lastIndexOf(b);
  if (s < 0 || e < s) fail("review-notes.md", "Notes markers missing");
  else {
    const notes = t.slice(s + a.length, e).trim();
    const n = chars(notes);
    const placeholders = notes.match(/\{\{[A-Z_]+\}\}/g) ?? [];
    // Unreplaced placeholders: keep room for a signed invite link (~150) + a video URL.
    const max = placeholders.length ? 3600 : 4000;
    console.log(`review-notes.md\n  ${"notes".padEnd(16)} ${String(n).padStart(4)} / ${max} chars${placeholders.length ? ` (placeholders: ${placeholders.join(" ")})` : ""}`);
    if (n > max) fail("review-notes.md", `Notes are ${n} chars (> ${max})`);
  }
}

console.log(failed ? `\n${failed} problem(s)` : "\nall listing checks passed");
process.exit(failed ? 1 : 0);
