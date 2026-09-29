import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from "vitest";
import { create, fromBinary, toBinary } from "@bufbuild/protobuf";
import { SignalSchema, WsTag, type Signal } from "@titi/protocol";
import { RelayTransport, type RelayEvents } from "../src/web/relay";

class FakeWs {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSING = 2;
  static readonly CLOSED = 3;
  static all: FakeWs[] = [];
  readyState = FakeWs.CONNECTING;
  binaryType = "blob";
  sent: Uint8Array[] = [];
  closed: { code?: number; reason?: string } | null = null;
  onopen: (() => void) | null = null;
  onmessage: ((e: { data: ArrayBuffer }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  constructor(readonly url: string) { FakeWs.all.push(this); }
  send(b: Uint8Array) { this.sent.push(b); }
  close(code?: number, reason?: string) { this.closed = { code, reason }; this.readyState = FakeWs.CLOSED; }
  // test helpers
  open() { this.readyState = FakeWs.OPEN; this.onopen?.(); }
  drop() { this.readyState = FakeWs.CLOSED; this.onclose?.(); }
  recv(b: Uint8Array) { const ab = new ArrayBuffer(b.length); new Uint8Array(ab).set(b); this.onmessage?.({ data: ab }); }
  signals(): Signal[] { return this.sent.filter((b) => b[0] === WsTag.Signal).map((b) => fromBinary(SignalSchema, b.subarray(1))); }
}

const bytes = (h: string) => Uint8Array.from(h.match(/.{2}/g)!, (b) => parseInt(b, 16));
const hex = (b: Uint8Array) => Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
const ME = bytes("aa".repeat(8));
const BOB = "bb".repeat(8);
const CAT = "cc".repeat(8);
const ROOM_A = bytes("01020304");
const ROOM_B = bytes("0a0b0c0d");

const sig = (kind: Parameters<typeof create<typeof SignalSchema>>[1]) => {
  const body = toBinary(SignalSchema, create(SignalSchema, kind));
  const out = new Uint8Array(body.length + 1);
  out[0] = WsTag.Signal;
  out.set(body, 1);
  return out;
};
const joinedFrame = (peers: string[], token = bytes("feed")) => sig({ kind: { case: "roomJoined", value: { resumeToken: token, peers: peers.map((p) => ({ nodeId: bytes(p), displayName: "x" })), roomSize: peers.length + 1 } } });
const peerEvent = (node: string, joined: boolean) => sig({ kind: { case: "peerEvent", value: { node: { nodeId: bytes(node) }, joined } } });
const envelope = (src: string, len = 32) => {
  const env = new Uint8Array(len);
  if (len >= 16) env.set(bytes(src), 8);
  const out = new Uint8Array(len + 1);
  out[0] = WsTag.Envelope;
  out.set(env, 1);
  return out;
};

type Ev = { [K in keyof Required<RelayEvents>]: Mock<NonNullable<RelayEvents[K]>> };
let ev: Ev;
let relay: RelayTransport;
const ws = () => FakeWs.all.at(-1)!;

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(1_000_000);
  FakeWs.all = [];
  vi.stubGlobal("WebSocket", FakeWs);
  vi.stubGlobal("window", globalThis);
  vi.spyOn(console, "warn").mockImplementation(() => {});
  ev = { linkUp: vi.fn<RelayEvents["linkUp"]>(), linkDown: vi.fn<RelayEvents["linkDown"]>(), peerSeen: vi.fn<RelayEvents["peerSeen"]>(), peerLost: vi.fn<RelayEvents["peerLost"]>(), frame: vi.fn<RelayEvents["frame"]>(), webrtc: vi.fn<NonNullable<RelayEvents["webrtc"]>>() };
  relay = new RelayTransport(() => "wss://relay.test/v1/ws", () => ME, () => "Me", () => 42, ev);
});

afterEach(() => {
  relay.stop();
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const connectAndJoin = (peers: string[] = []) => {
  relay.setRooms([{ hash: ROOM_A, rendezvous: false }]);
  relay.start();
  ws().open();
  ws().recv(joinedFrame(peers));
};

describe("connect and join", () => {
  it("opens one binary socket to the configured URL on start", () => {
    relay.start();
    expect(FakeWs.all).toHaveLength(1);
    expect(ws().url).toBe("wss://relay.test/v1/ws");
    expect(ws().binaryType).toBe("arraybuffer");
  });

  it("sends a roomJoin per desired room on open, carrying node identity and rendezvous flag", () => {
    relay.setRooms([{ hash: ROOM_A, rendezvous: false }, { hash: ROOM_B, rendezvous: true }]);
    relay.start();
    expect(ws().sent).toHaveLength(0);
    ws().open();
    const joins = ws().signals().filter((s) => s.kind.case === "roomJoin").map((s) => s.kind.value as { groupHash: Uint8Array; rendezvous: boolean; node?: { nodeId: Uint8Array; displayName: string; avatarHue: number } });
    expect(joins.map((j) => [hex(j.groupHash), j.rendezvous])).toEqual([["01020304", false], ["0a0b0c0d", true]]);
    expect(hex(joins[0]!.node!.nodeId)).toBe("aa".repeat(8));
    expect(joins[0]!.node!.displayName).toBe("Me");
    expect(joins[0]!.node!.avatarHue).toBe(42);
  });

  it("raises linkUp once on the first roomJoined and reports the listed peers", () => {
    connectAndJoin([BOB]);
    ws().recv(joinedFrame([BOB, CAT]));
    expect(ev.linkUp).toHaveBeenCalledTimes(1);
    expect(ev.peerSeen.mock.calls.map((c) => c[0])).toEqual([BOB, CAT]);
  });

  it("setRooms while connected joins new rooms and leaves dropped ones", () => {
    connectAndJoin();
    ws().sent = [];
    relay.setRooms([{ hash: ROOM_B, rendezvous: true }]);
    const s = ws().signals();
    expect(s.map((x) => x.kind.case)).toEqual(["roomLeave", "roomJoin"]);
    expect(hex((s[0]!.kind.value as { groupHash: Uint8Array }).groupHash)).toBe("01020304");
    expect(hex((s[1]!.kind.value as { groupHash: Uint8Array }).groupHash)).toBe("0a0b0c0d");
  });

  it("setRooms with an identical set sends nothing", () => {
    connectAndJoin();
    ws().sent = [];
    relay.setRooms([{ hash: ROOM_A, rendezvous: false }]);
    expect(ws().sent).toHaveLength(0);
  });

  it("setRooms before the socket opens only records the rooms", () => {
    relay.start();
    relay.setRooms([{ hash: ROOM_A, rendezvous: false }]);
    expect(ws().sent).toHaveLength(0);
  });

  it("re-joins all rooms with a cleared resume token after relay error 5", () => {
    connectAndJoin();
    ws().sent = [];
    ws().recv(sig({ kind: { case: "error", value: { code: 5, message: "unknown session" } } }));
    const joins = ws().signals().filter((s) => s.kind.case === "roomJoin");
    expect(joins).toHaveLength(1);
    expect((joins[0]!.kind.value as { resumeToken: Uint8Array }).resumeToken.length).toBe(0);
    expect(console.warn).toHaveBeenCalledWith("relay error", 5, "unknown session");
  });

  it("other relay errors are only logged", () => {
    connectAndJoin();
    ws().sent = [];
    ws().recv(sig({ kind: { case: "error", value: { code: 6, message: "frame too large" } } }));
    expect(ws().sent).toHaveLength(0);
  });

  it("presents the last resume token on the next join", () => {
    relay.setRooms([{ hash: ROOM_A, rendezvous: false }]);
    relay.start();
    ws().open();
    ws().recv(joinedFrame([], bytes("c0ffee")));
    ws().drop();
    vi.advanceTimersByTime(1000);
    ws().open();
    const j = ws().signals().find((s) => s.kind.case === "roomJoin")!;
    expect(hex((j.kind.value as { resumeToken: Uint8Array }).resumeToken)).toBe("c0ffee");
  });
});

describe("incoming frames", () => {
  it("dispatches envelopes as frames from the src node id at bytes 8..16, announcing new peers once", () => {
    connectAndJoin();
    ws().recv(envelope(BOB));
    ws().recv(envelope(BOB));
    expect(ev.peerSeen).toHaveBeenCalledTimes(1);
    expect(ev.peerSeen).toHaveBeenCalledWith(BOB);
    expect(ev.frame).toHaveBeenCalledTimes(2);
    const [src, body] = ev.frame.mock.calls[0]! as [string, Uint8Array];
    expect(src).toBe(BOB);
    expect(body.length).toBe(32);
  });

  it("ignores empty frames, short envelopes, unknown tags and undecodable signals", () => {
    connectAndJoin();
    ws().recv(new Uint8Array());
    ws().recv(envelope(BOB, 10));
    ws().recv(Uint8Array.of(0x07, 1, 2));
    ws().recv(Uint8Array.of(WsTag.Signal, 0xff, 0xff, 0xff));
    ws().recv(sig({ kind: { case: "pong", value: { tsMs: 1n, serverMs: 2n } } }));
    expect(ev.frame).not.toHaveBeenCalled();
    expect(ev.peerSeen).not.toHaveBeenCalled();
  });

  it("peerEvent joined announces a peer; left reports it lost only if known", () => {
    connectAndJoin();
    ws().recv(peerEvent(CAT, true));
    ws().recv(peerEvent(CAT, true));
    ws().recv(peerEvent(CAT, false));
    ws().recv(peerEvent(BOB, false));
    ws().recv(sig({ kind: { case: "peerEvent", value: { joined: true } } }));
    expect(ev.peerSeen.mock.calls).toEqual([[CAT]]);
    expect(ev.peerLost.mock.calls).toEqual([[CAT]]);
  });

  it("forwards WebRTC signalling by payload kind", () => {
    connectAndJoin();
    const w = (payload: { case: "offerSdp" | "answerSdp" | "iceCandidate"; value: string }) => sig({ kind: { case: "webrtc", value: { to: ME, from: bytes(BOB), payload } } });
    ws().recv(w({ case: "offerSdp", value: "o" }));
    ws().recv(w({ case: "answerSdp", value: "a" }));
    ws().recv(w({ case: "iceCandidate", value: "i" }));
    expect(ev.webrtc.mock.calls).toEqual([
      [BOB, { offerSdp: "o", answerSdp: undefined, ice: undefined }],
      [BOB, { offerSdp: undefined, answerSdp: "a", ice: undefined }],
      [BOB, { offerSdp: undefined, answerSdp: undefined, ice: "i" }],
    ]);
  });
});

describe("outgoing", () => {
  it("send prefixes the envelope tag only once joined", () => {
    relay.setRooms([{ hash: ROOM_A, rendezvous: false }]);
    relay.start();
    ws().open();
    ws().sent = [];
    relay.send(Uint8Array.of(9, 8));
    expect(ws().sent).toHaveLength(0);
    ws().recv(joinedFrame([]));
    relay.send(Uint8Array.of(9, 8));
    expect(Array.from(ws().sent[0]!)).toEqual([WsTag.Envelope, 9, 8]);
  });

  it("sendWebRtc encodes offer, answer and ice with our node id as sender", () => {
    connectAndJoin();
    ws().sent = [];
    const to = bytes(BOB);
    relay.sendWebRtc(to, { offerSdp: "o" });
    relay.sendWebRtc(to, { answerSdp: "a" });
    relay.sendWebRtc(to, { ice: "i" });
    relay.sendWebRtc(to, {});
    const v = ws().signals().map((s) => s.kind.value as { to: Uint8Array; from: Uint8Array; payload: { case: string; value: string } });
    expect(v.map((x) => [x.payload.case, x.payload.value])).toEqual([["offerSdp", "o"], ["answerSdp", "a"], ["iceCandidate", "i"], ["iceCandidate", ""]]);
    expect(hex(v[0]!.to)).toBe(BOB);
    expect(hex(v[0]!.from)).toBe("aa".repeat(8));
  });

  it("sendWebRtc is dropped while the socket is not open", () => {
    relay.start();
    relay.sendWebRtc(bytes(BOB), { offerSdp: "o" });
    expect(ws().sent).toHaveLength(0);
  });
});

describe("reconnect", () => {
  it("on close reports every known peer lost, then linkDown, then reconnects with exponential backoff", () => {
    connectAndJoin([BOB, CAT]);
    ws().drop();
    expect(ev.peerLost.mock.calls).toEqual([[BOB], [CAT]]);
    expect(ev.linkDown).toHaveBeenCalledTimes(1);
    expect(FakeWs.all).toHaveLength(1);
    vi.advanceTimersByTime(999);
    expect(FakeWs.all).toHaveLength(1);
    vi.advanceTimersByTime(1);
    expect(FakeWs.all).toHaveLength(2); // attempt 1: 1000 ms
    ws().drop(); // never opened: no linkDown again
    expect(ev.linkDown).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(1999);
    expect(FakeWs.all).toHaveLength(2);
    vi.advanceTimersByTime(1);
    expect(FakeWs.all).toHaveLength(3); // attempt 2: 2000 ms
  });

  it("caps the backoff at 30 s and resets it after a successful open", () => {
    relay.start();
    for (let i = 0; i < 8; i++) { ws().drop(); vi.advanceTimersByTime(30_000); }
    expect(FakeWs.all).toHaveLength(9);
    ws().drop();
    vi.advanceTimersByTime(29_999);
    expect(FakeWs.all).toHaveLength(9);
    vi.advanceTimersByTime(1);
    expect(FakeWs.all).toHaveLength(10);
    ws().open();
    ws().drop();
    vi.advanceTimersByTime(1000);
    expect(FakeWs.all).toHaveLength(11);
  });

  it("stop closes the socket with 1000, reports linkDown and never reconnects", () => {
    connectAndJoin();
    const s = ws();
    relay.stop();
    expect(s.closed).toEqual({ code: 1000, reason: "bye" });
    expect(ev.linkDown).toHaveBeenCalledTimes(1);
    s.drop();
    vi.advanceTimersByTime(120_000);
    expect(FakeWs.all).toHaveLength(1);
  });

  it("stop cancels a pending reconnect", () => {
    relay.start();
    ws().drop();
    relay.stop();
    vi.advanceTimersByTime(60_000);
    expect(FakeWs.all).toHaveLength(1);
    expect(ev.linkDown).not.toHaveBeenCalled();
  });
});

describe("heartbeat", () => {
  it("pings every 15 s while traffic keeps arriving", () => {
    connectAndJoin();
    ws().sent = [];
    vi.advanceTimersByTime(15_000);
    const pings = ws().signals().filter((s) => s.kind.case === "ping");
    expect(pings).toHaveLength(1);
    expect((pings[0]!.kind.value as { tsMs: bigint }).tsMs).toBe(BigInt(Date.now()));
  });

  it("does not ping before the socket opens", () => {
    relay.start();
    vi.advanceTimersByTime(15_000);
    expect(ws().sent).toHaveLength(0);
  });

  it("reconnects when nothing arrived for over 40 s (half-open socket)", () => {
    connectAndJoin([BOB]);
    const first = ws();
    vi.advanceTimersByTime(45_000); // beats at 15/30 ping, at 45 s silence > 40 s
    expect(first.closed).not.toBeNull();
    expect(first.onclose).toBeNull();
    expect(ev.peerLost).toHaveBeenCalledWith(BOB);
    expect(ev.linkDown).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(1000);
    expect(FakeWs.all).toHaveLength(2);
    expect(console.warn).toHaveBeenCalledWith("relay: no traffic for 40 s, reconnecting");
  });

  it("start twice keeps a single heartbeat", () => {
    connectAndJoin();
    relay.start(); // second connect() opens another socket; heartbeat must not double
    ws().open();
    ws().recv(joinedFrame([]));
    ws().sent = [];
    vi.advanceTimersByTime(15_000);
    expect(ws().signals().filter((s) => s.kind.case === "ping")).toHaveLength(1);
  });
});
