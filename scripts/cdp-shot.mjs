// Screenshot a WebView2/Chromium page over CDP. usage: node scripts/cdp-shot.mjs <out.png> [port] [urlSubstring] [delayMs]
import { writeFileSync } from "node:fs";
const [out = ".copilot-tmp/cdp.png", port = "9223", match = "", delay = "0"] = process.argv.slice(2);
const list = await (await fetch(`http://localhost:${port}/json`)).json();
const page = list.find((p) => p.type === "page" && p.url.includes(match)) ?? list[0];
if (!page) throw new Error("no debuggable page");
await new Promise((r) => setTimeout(r, Number(delay)));
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
ws.send(JSON.stringify({ id: 1, method: "Page.captureScreenshot", params: { format: "png" } }));
const res = await new Promise((r) => { ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id === 1) r(d.result); }; });
writeFileSync(out, Buffer.from(res.data, "base64"));
console.log(`saved ${out} from ${page.url}`);
ws.close();
