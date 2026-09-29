// Screenshot the Tizen app via CDP. usage: node scripts/tv-shot.mjs out.png [port]
import { writeFileSync } from "node:fs";
const out = process.argv[2] ?? ".copilot-tmp/tv.png";
const port = process.argv[3] ?? "9222";
const list = await (await fetch(`http://localhost:${port}/json`)).json();
const page = list.find((p) => p.type === "page") ?? list[0];
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
ws.send(JSON.stringify({ id: 1, method: "Page.captureScreenshot", params: { format: "png" } }));
const res = await new Promise((r) => { ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id === 1) r(d.result); }; });
writeFileSync(out, Buffer.from(res.data, "base64"));
console.log("saved", out);
ws.close();
