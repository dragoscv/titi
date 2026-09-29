// Vendor only the Material icons the apps use (instead of material-icons-extended: 34 MB AAR,
// ~11k classes, 40 MB of debug dex). Copies the official Apache-2.0 sources unchanged, so the
// `androidx.compose.material.icons.*` imports keep working.
// usage: node scripts/vendor-icons.mjs <icons-extended-sources.jar> <icons-core-sources.jar>
// Icons already in material-icons-core (a dependency) are skipped: vendoring them would clash.
import { execFileSync } from "node:child_process";
import { mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const root = new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const [jar, coreJar] = process.argv.slice(2);
if (!jar || !coreJar) throw new Error("pass the icons-extended and icons-core sources jars");
const coreList = execFileSync("tar", ["-tf", coreJar], { encoding: "utf8" });
const out = join(root, "android/client/src/main/kotlin/androidx/compose/material/icons");

const walk = (d) => readdirSync(d).flatMap((f) => { const p = join(d, f); return statSync(p).isDirectory() ? walk(p) : p.endsWith(".kt") ? [p] : []; });
const used = new Set();
for (const m of ["app", "wear", "tv", "client"]) {
  for (const f of walk(join(root, "android", m, "src"))) {
    if (f.startsWith(out)) continue;
    for (const x of readFileSync(f, "utf8").matchAll(/Icons\.(AutoMirrored\.)?(Rounded|Filled|Outlined)\.(\w+)/g)) used.add(`${x[1] ? "automirrored/" : ""}${x[2].toLowerCase()}/${x[3]}`);
  }
}
const tmp = join(root, ".copilot-tmp/icons-src");
rmSync(tmp, { recursive: true, force: true });
mkdirSync(tmp, { recursive: true });
execFileSync("tar", ["-xf", jar, "-C", tmp]);
rmSync(out, { recursive: true, force: true });
const base = join(tmp, "commonMain/androidx/compose/material/icons");
let skipped = 0;
for (const u of [...used].sort()) {
  if (coreList.includes(`icons/${u}.kt`)) { skipped++; continue; }
  const src = join(base, `${u}.kt`);
  const dst = join(out, `${u}.kt`);
  mkdirSync(join(dst, ".."), { recursive: true });
  writeFileSync(dst, readFileSync(src, "utf8"));
}
console.log(`vendored ${used.size - skipped} icons (${skipped} come from icons-core) -> ${relative(root, out)}`);
