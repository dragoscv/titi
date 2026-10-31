#!/usr/bin/env node
// One-off: create the App Store review demo group for the relay's demo bot
// (apps/backend/src/demo-bot.ts). Generates a fresh identity + group
// "Titi Demo", prints the public invite link (2-year validity) and writes the
// SECRETS to .copilot-tmp/demo-bot/ (gitignored):
//   seed.hex   -> DEMO_BOT_SEED   (identity seed; never printed)
//   groups.hex -> DEMO_BOT_GROUPS (engine "groups" blob; holds the group key)
//   link.txt   -> the invite links (public by design)
// Usage: node scripts/demo-group-init.mjs [--force]
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const outDir = join(root, ".copilot-tmp", "demo-bot");
const force = process.argv.includes("--force");
if (existsSync(join(outDir, "seed.hex")) && !force) {
  console.error(`refusing to overwrite ${outDir}\\seed.hex (pass --force to create a NEW demo group)`);
  process.exit(1);
}

const wasm = createRequire(import.meta.url)(join(root, "packages", "core-wasm", "pkg-node", "titi_wasm.js"));
const hex = (b) => Buffer.from(b).toString("hex");
const now = Date.now();
const TWO_YEARS_MS = 2 * 365 * 24 * 60 * 60_000;

const eng = new wasm.WasmEngine(new Uint8Array(), "Titi Demo", 200, BigInt(Math.floor(Math.random() * 2 ** 52)));
const acts = JSON.parse(eng.create_group("Titi Demo", now));
const groups = acts.find((a) => a.t === "persist" && a.key === "groups");
const joined = acts.find((a) => a.t === "ui" && a.event.type === "joined");
if (!groups || !joined) throw new Error("create_group produced no groups blob");
const link = eng.deep_link(Uint8Array.from(Buffer.from(joined.event.group, "hex")), now, TWO_YEARS_MS);
if (!link) throw new Error("deep_link failed");
const https = link.replace("titi://j/", "https://titi.dragoscatalin.ro/j/");

// sanity: the blob restores into a fresh engine with the same seed
const check = new wasm.WasmEngine(eng.identity_seed(), "Titi Demo", 200, 1n);
if (!check.restore_groups(Uint8Array.from(groups.value), now) || JSON.parse(check.groups_json()).length !== 1) {
  throw new Error("groups blob does not restore");
}

mkdirSync(outDir, { recursive: true });
writeFileSync(join(outDir, "seed.hex"), hex(eng.identity_seed()), { mode: 0o600 });
writeFileSync(join(outDir, "groups.hex"), hex(Uint8Array.from(groups.value)), { mode: 0o600 });
writeFileSync(join(outDir, "link.txt"), `${link}\n${https}\n`);

console.log(`group     ${joined.event.name} (${joined.event.group})`);
console.log(`expires   ${new Date(now + TWO_YEARS_MS).toISOString()}`);
console.log(`link      ${link}`);
console.log(`web link  ${https}`);
console.log(`secrets   ${join(outDir, "seed.hex")} (DEMO_BOT_SEED), ${join(outDir, "groups.hex")} (DEMO_BOT_GROUPS)`);
