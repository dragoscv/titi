// Evaluate a JS expression in the Tizen app via the forwarded DevTools port.
// usage: node scripts/tv-eval.mjs "<expr>" [port] [url-regex]
//        node scripts/tv-eval.mjs @file.js [port] [url-regex]   (avoids shell quoting)
// Gives up after TV_EVAL_TIMEOUT_MS (default 20 s) so a sleeping TV cannot hang the caller.
import { readFileSync } from "node:fs";
const arg = process.argv[2] ?? "document.title";
const expr = arg.startsWith("@") ? readFileSync(arg.slice(1), "utf8") : arg;
const timeout = Number(process.env.TV_EVAL_TIMEOUT_MS ?? 20000);
setTimeout(() => { console.error(`tv-eval: no answer in ${timeout} ms (target asleep?)`); process.exit(2); }, timeout).unref();
const port = process.argv[3] ?? "9222";
const match = process.argv[4] ? new RegExp(process.argv[4]) : null;
const list = await (await fetch(`http://localhost:${port}/json`)).json();
const page = list.find((p) => p.type === "page" && (!match || match.test(p.url))) ?? list[0];
if (!page) throw new Error("no debuggable page");
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
ws.send(JSON.stringify({ id: 1, method: "Runtime.evaluate", params: { expression: expr, awaitPromise: true, returnByValue: true } }));
const res = await new Promise((r) => { ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id === 1) r(d.result); }; });
console.log(typeof res?.result?.value === "string" ? res.result.value : JSON.stringify(res, null, 1));
ws.close();
