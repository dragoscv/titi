import { fromBinary } from "@bufbuild/protobuf";
import { serve, type ServerType } from "@hono/node-server";
import { SignalSchema, WsTag, type Signal } from "@titi/protocol";
import type { AddressInfo } from "node:net";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { WebSocket } from "ws";
import { createApp } from "../src/app.ts";
import { loadConfig } from "../src/config.ts";
import { setLogLevel } from "../src/log.ts";
import { Relay } from "../src/relay.ts";
import { envelope, join, nodeA, nodeB, signal } from "./helpers.ts";

let server: ServerType;
let relay: Relay;
let base = "";

beforeAll(async () => {
  setLogLevel("error");
  const cfg = loadConfig({});
  relay = new Relay(cfg);
  const { app, injectWebSocket } = createApp(cfg, relay);
  await new Promise<void>((resolve) => {
    server = serve({ fetch: app.fetch, port: 0, hostname: "127.0.0.1" }, () => resolve());
    injectWebSocket(server);
  });
  const addr = server.address();
  if (!isAddressInfo(addr)) throw new Error("server has no TCP address");
  base = `127.0.0.1:${addr.port}`;
});

function isAddressInfo(a: unknown): a is AddressInfo {
  return typeof a === "object" && a !== null && "port" in a;
}

async function jsonBody(res: Response): Promise<Record<string, unknown>> {
  const v: unknown = await res.json();
  if (typeof v !== "object" || v === null) throw new Error("expected a JSON object");
  return { ...v };
}

afterAll(async () => {
  setLogLevel("info");
  await new Promise<void>((r) => server.close(() => r()));
});

/** A ws client that queues every binary frame so tests can await the next one. */
class Client {
  private queue: Uint8Array[] = [];
  private waiters: ((b: Uint8Array) => void)[] = [];
  readonly ws: WebSocket;
  readonly opened: Promise<void>;
  constructor() {
    this.ws = new WebSocket(`ws://${base}/v1/ws`);
    this.ws.binaryType = "nodebuffer";
    this.ws.on("message", (d: Buffer) => {
      const b = new Uint8Array(d);
      const w = this.waiters.shift();
      if (w) w(b);
      else this.queue.push(b);
    });
    this.opened = new Promise((res, rej) => {
      this.ws.once("open", () => res());
      this.ws.once("error", rej);
    });
  }
  next(timeoutMs = 2000): Promise<Uint8Array> {
    const q = this.queue.shift();
    if (q) return Promise.resolve(q);
    return new Promise((res, rej) => {
      const t = setTimeout(() => rej(new Error("timeout waiting for frame")), timeoutMs);
      this.waiters.push((b) => {
        clearTimeout(t);
        res(b);
      });
    });
  }
  async nextSignal(): Promise<Signal> {
    const b = await this.next();
    expect(b[0]).toBe(WsTag.Signal);
    return fromBinary(SignalSchema, b.subarray(1));
  }
  close(): Promise<void> {
    return new Promise((r) => {
      this.ws.once("close", () => r());
      this.ws.close();
    });
  }
}

async function until(cond: () => boolean) {
  for (let i = 0; i < 100 && !cond(); i++) await new Promise((r) => setTimeout(r, 10));
}

describe("http app", () => {
  it("GET /health returns ok, service name, version and relay stats", async () => {
    const res = await fetch(`http://${base}/health`);
    expect(res.status).toBe(200);
    const body = await jsonBody(res);
    expect(body).toMatchObject({ ok: true, service: "titi-relay", rooms: 0, members: 0, connections: 0, parked: 0 });
    expect(typeof body.version).toBe("string");
  });

  it("unknown routes are 404", async () => {
    const res = await fetch(`http://${base}/nope`);
    expect(res.status).toBe(404);
  });

  it("relays an envelope between two websocket clients and cleans up on close", async () => {
    const a = new Client();
    const b = new Client();
    await Promise.all([a.opened, b.opened]);
    a.ws.send(join(nodeA));
    expect((await a.nextSignal()).kind.case).toBe("roomJoined");
    b.ws.send(join(nodeB));
    const bJoined = await b.nextSignal();
    expect(bJoined.kind.case === "roomJoined" && bJoined.kind.value.roomSize).toBe(2);
    expect((await a.nextSignal()).kind.case).toBe("peerEvent");

    // text frames are ignored: next thing A sees must be the pong, not an error
    a.ws.send("hello");
    a.ws.send(signal({ case: "ping", value: { tsMs: 5n } }));
    const pong = await a.nextSignal();
    expect(pong.kind.case === "pong" && pong.kind.value.tsMs).toBe(5n);

    const frame = envelope(nodeA);
    a.ws.send(frame);
    expect(await b.next()).toEqual(frame);

    const health = await jsonBody(await fetch(`http://${base}/health`));
    expect(health).toMatchObject({ rooms: 1, members: 2, connections: 2 });

    await b.close();
    const left = await a.nextSignal();
    expect(left.kind.case === "peerEvent" && left.kind.value.joined).toBe(false);
    await a.close();
    await until(() => relay.stats().connections === 0);
    expect(relay.stats()).toMatchObject({ rooms: 0, members: 0, connections: 0, parked: 2 });
  });

  it("closes the stale socket with 4000 when the same node connects again", async () => {
    const a1 = new Client();
    await a1.opened;
    a1.ws.send(join(nodeA));
    await a1.nextSignal();
    const closed = new Promise<number>((r) => a1.ws.once("close", (code) => r(code)));
    const a2 = new Client();
    await a2.opened;
    a2.ws.send(join(nodeA));
    expect((await a2.nextSignal()).kind.case).toBe("roomJoined");
    expect(await closed).toBe(4000);
    await a2.close();
  });
});
