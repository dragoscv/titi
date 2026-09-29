// Reload the Tizen page and stream console + exceptions for N seconds.
// usage: node scripts/tv-console.mjs [seconds] [port]
const secs = Number(process.argv[2] ?? 8);
const port = process.argv.slice(3).find((a) => !a.startsWith("--")) ?? "9222";
const list = await (await fetch(`http://localhost:${port}/json`)).json();
const page = list.find((p) => p.type === "page") ?? list[0];
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
let id = 0;
const send = (method, params = {}) => ws.send(JSON.stringify({ id: ++id, method, params }));
ws.onmessage = (m) => {
  const d = JSON.parse(m.data);
  if (d.method === "Runtime.consoleAPICalled") console.log(`[${d.params.type}]`, d.params.args.map((a) => a.value ?? a.description ?? a.type).join(" "));
  if (d.method === "Runtime.exceptionThrown") console.log("[exception]", d.params.exceptionDetails.exception?.description ?? d.params.exceptionDetails.text);
  if (d.method === "Log.entryAdded") console.log(`[log:${d.params.entry.level}]`, d.params.entry.text, d.params.entry.url ?? "");
};
send("Runtime.enable");
send("Log.enable");
if (!process.argv.includes("--no-reload")) send("Page.reload", { ignoreCache: true });
await new Promise((r) => setTimeout(r, secs * 1000));
ws.close();
