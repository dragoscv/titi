#!/usr/bin/env node
// Report version-catalog entries behind the latest STABLE release (Google Maven + Maven Central).
// Usage: node scripts/android-outdated.mjs   (exit 0 always; prints a table)
import { readFileSync } from "node:fs";

const toml = readFileSync(new URL("../android/gradle/libs.versions.toml", import.meta.url), "utf8");
const section = (name) => toml.split(`[${name}]`)[1]?.split(/^\[/m)[0] ?? "";
const versions = Object.fromEntries(
  [...section("versions").matchAll(/^(\w[\w-]*)\s*=\s*"([^"]+)"/gm)].map((m) => [m[1], m[2]]),
);
const refs = new Map();
for (const m of section("libraries").matchAll(/module\s*=\s*"([^"]+)".*?version\.ref\s*=\s*"([^"]+)"/g)) {
  if (!refs.has(m[2])) refs.set(m[2], m[1]);
}
for (const m of section("plugins").matchAll(/id\s*=\s*"([^"]+)".*?version\.ref\s*=\s*"([^"]+)"/g)) {
  if (!refs.has(m[2])) refs.set(m[2], `${m[1]}:${m[1]}.gradle.plugin`);
}

const stable = (v) => !/alpha|beta|rc|dev|snapshot|-m\d/i.test(v);
const cmp = (a, b) => {
  const pa = a.split(/[.-]/).map(Number), pb = b.split(/[.-]/).map(Number);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] || 0) - (pb[i] || 0);
    if (d) return d;
  }
  return 0;
};
async function latest(coord) {
  const [g, a] = coord.split(":");
  const path = `${g.replaceAll(".", "/")}/${a}/maven-metadata.xml`;
  for (const base of ["https://dl.google.com/dl/android/maven2", "https://repo1.maven.org/maven2", "https://plugins.gradle.org/m2"]) {
    const r = await fetch(`${base}/${path}`, { signal: AbortSignal.timeout(10_000) }).catch(() => null);
    if (!r?.ok) continue;
    const xml = await r.text();
    const all = [...xml.matchAll(/<version>([^<]+)<\/version>/g)].map((m) => m[1]).filter(stable);
    if (all.length) return all.sort(cmp).at(-1);
  }
  return null;
}

const rows = await Promise.all(
  [...refs].map(async ([ref, coord]) => ({ ref, coord, current: versions[ref], latest: await latest(coord) })),
);
const behind = rows.filter((r) => r.latest && r.current && cmp(r.latest, r.current) > 0);
console.table(behind.map(({ ref, current, latest }) => ({ ref, current, latest })));
console.log(`${behind.length} behind / ${rows.length} checked; unresolved: ${rows.filter((r) => !r.latest).map((r) => r.ref).join(", ") || "none"}`);
