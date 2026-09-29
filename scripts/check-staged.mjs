#!/usr/bin/env node
// pre-commit: refuse large blobs and build output / signing material that must never be committed.
import { statSync } from "node:fs";

const MAX_KB = 1024;
// committed on purpose: the prebuilt wasm (so web/Vercel builds need no Rust toolchain)
const ALLOW_LARGE = [/^packages\/core-wasm\/pkg\//];
const FORBID = [/\.(jks|keystore|p12|pem|pfx|aab|apk|ipa|msix)$/i, /(^|\/)(target|build|dist|\.next)\//, /(^|\/)\.env(\.|$)(?!example)/, /service-account.*\.json$/i, /google-services\.json$/];

let bad = 0;
for (const f of process.argv.slice(2).map((p) => p.replaceAll("\\", "/"))) {
  if (FORBID.some((r) => r.test(f))) { console.error(`forbidden path staged: ${f}`); bad++; continue; }
  let kb = 0;
  try { kb = statSync(f).size / 1024; } catch { continue; } // deleted file
  if (kb > MAX_KB && !ALLOW_LARGE.some((r) => r.test(f))) { console.error(`too large (${kb.toFixed(0)} KB > ${MAX_KB} KB): ${f}`); bad++; }
}
process.exit(bad ? 1 : 0);
