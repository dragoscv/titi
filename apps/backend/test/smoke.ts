// End-to-end smoke: starts dist/index.js, connects two ws clients, joins a
// room, sends an envelope A→B and checks it arrives. Run: pnpm tsx test/smoke.ts
import { create, fromBinary, toBinary } from "@bufbuild/protobuf";
import { PROTOCOL_VERSION, SignalSchema, WsTag } from "@titi/protocol";
import { spawn } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";
import WebSocket from "ws";

const PORT = 18080;
const server = spawn(process.execPath, ["dist/index.js"], { env: { ...process.env, PORT: String(PORT), LOG_LEVEL: "debug" }, stdio: ["ignore", "pipe", "inherit"] });
server.stdout.on("data", (d) => process.stdout.write(`[server] ${d}`));

try {
  for (let i = 0; i < 50; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${PORT}/health`);
      if (r.ok) {
        console.log("health:", await r.text());
        break;
      }
    } catch {}
    await sleep(100);
  }

  const join = (node: number) => {
    const s = create(SignalSchema, {
      kind: { case: "roomJoin", value: { groupHash: new Uint8Array([1, 2, 3, 4]), node: { nodeId: new Uint8Array([node, 1, 2, 3, 4, 5, 6, 7]), displayName: `n${node}` } } },
    });
    const body = toBinary(SignalSchema, s);
    const out = new Uint8Array(body.length + 1);
    out[0] = WsTag.Signal;
    out.set(body, 1);
    return out;
  };
  const open = (n: number) =>
    new Promise<WebSocket>((res, rej) => {
      const ws = new WebSocket(`ws://127.0.0.1:${PORT}/v1/ws`);
      ws.binaryType = "nodebuffer";
      ws.once("open", () => {
        ws.send(join(n));
        res(ws);
      });
      ws.once("error", rej);
    });

  const a = await open(0xa);
  const b = await open(0xb);
  const gotB = new Promise<Uint8Array>((res) => {
    b.on("message", (d: Buffer) => {
      if (d[0] === WsTag.Envelope) res(new Uint8Array(d.subarray(1)));
      else console.log("B signal:", fromBinary(SignalSchema, new Uint8Array(d.subarray(1))).kind.case);
    });
  });
  await sleep(200);
  const env = new Uint8Array(1 + 16 + 3);
  env[0] = WsTag.Envelope;
  env[1] = PROTOCOL_VERSION;
  env[2] = 0x11;
  env[3] = 0x33;
  env.set([0xa, 1, 2, 3, 4, 5, 6, 7], 1 + 8);
  env.set([7, 7, 7], 1 + 16);
  a.send(env);
  const got = await Promise.race([gotB, sleep(2000).then(() => null)]);
  if (!got || got[16] !== 7) throw new Error("B did not receive A's envelope");
  console.log("OK: B received envelope from A, bytes =", got.length);
  a.close();
  b.close();
  await sleep(100);
  console.log("health after:", await (await fetch(`http://127.0.0.1:${PORT}/health`)).text());
} finally {
  server.kill();
}
