import { FrameType, nodeIdHex, WsTag } from "@titi/protocol";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { loadConfig } from "../src/config.ts";
import { setLogLevel } from "../src/log.ts";
import { ErrorCode, Relay, tagged } from "../src/relay.ts";
import { envelope, FakeSink, GH, GH2, join, nodeA, nodeB, nodeC, signal } from "./helpers.ts";

function setup(env: Record<string, string> = {}) {
  let t = 1_000_000;
  const cfg = loadConfig({ RATE_LIMIT_FPS: "5", RESUME_TTL_MS: "1000", MAX_ROOM_SIZE: "3", MAX_FRAME_BYTES: "64", ...env });
  const relay = new Relay(cfg, () => t);
  const peer = (node?: Uint8Array, opts?: Parameters<typeof join>[1]) => {
    const sink = new FakeSink();
    const id = relay.connect(sink);
    if (node) relay.onMessage(id, join(node, opts));
    return { sink, id };
  };
  return { relay, peer, tick: (ms: number) => (t += ms), now: () => t };
}

beforeEach(() => setLogLevel("error"));
afterEach(() => setLogLevel("info"));

/** Relay on the real (fake-timer controlled) Date.now clock. */
function clockRelay() {
  const relay = new Relay(loadConfig({ RESUME_TTL_MS: "1000" }));
  const peer = (node?: Uint8Array, opts?: Parameters<typeof join>[1]) => {
    const sink = new FakeSink();
    const id = relay.connect(sink);
    if (node) relay.onMessage(id, join(node, opts));
    return { sink, id };
  };
  return { relay, peer };
}

const rtc = (to: Uint8Array) =>
  signal({ case: "webrtc", value: { to, from: nodeC /* spoofed */, payload: { case: "offerSdp", value: "v=0" } } });

describe("relay frame validation", () => {
  it("ignores messages for unknown connection ids and empty frames", () => {
    const { relay, peer } = setup();
    expect(() => relay.onMessage(999, join(nodeA))).not.toThrow();
    expect(relay.stats()).toEqual({ rooms: 0, members: 0, connections: 0, parked: 0 });
    const a = peer();
    relay.onMessage(a.id, new Uint8Array());
    expect(a.sink.out).toHaveLength(0);
  });

  it("accepts a frame of exactly MAX_FRAME_BYTES plus tag and rejects one byte more as TooLarge", () => {
    const { relay, peer } = setup();
    const a = peer();
    const ok = new Uint8Array(1 + 64); // tag + 64-byte body; unknown tag so we see a BadFrame, not TooLarge
    ok[0] = 0x7f;
    relay.onMessage(a.id, ok);
    expect(a.sink.errors()).toEqual([{ code: ErrorCode.BadFrame, message: "unknown tag" }]);
    const big = new Uint8Array(1 + 65);
    big[0] = WsTag.Envelope;
    relay.onMessage(a.id, big);
    expect(a.sink.errors()[1]).toEqual({ code: ErrorCode.TooLarge, message: "frame too large" });
  });

  it("answers garbage protobuf in a signal frame with BadFrame instead of throwing", () => {
    const { relay, peer } = setup();
    const a = peer();
    // field 1 with wire type 7 is invalid protobuf
    expect(() => relay.onMessage(a.id, new Uint8Array([WsTag.Signal, 0x0f, 0xff, 0xff]))).not.toThrow();
    expect(a.sink.errors()).toEqual([{ code: ErrorCode.BadFrame, message: "bad signal" }]);
    expect(a.sink.closed).toBeNull();
  });

  it("ignores a signal with no kind and client-sent server-only signals", () => {
    const { relay, peer } = setup();
    const a = peer();
    relay.onMessage(a.id, new Uint8Array([WsTag.Signal]));
    relay.onMessage(a.id, signal({ case: "pong", value: { tsMs: 1n, serverMs: 2n } }));
    expect(a.sink.out).toHaveLength(0);
  });

  it.each([
    ["group hash of 3 bytes", { groupHash: new Uint8Array([1, 2, 3]), node: { nodeId: nodeA } }],
    ["missing node", { groupHash: GH }],
    ["node id of 7 bytes", { groupHash: GH, node: { nodeId: nodeA.subarray(0, 7) } }],
  ])("rejects room_join with %s", (_label, value) => {
    const { relay, peer } = setup();
    const a = peer();
    relay.onMessage(a.id, signal({ case: "roomJoin", value }));
    expect(a.sink.errors()).toEqual([{ code: ErrorCode.BadFrame, message: "bad room_join" }]);
    expect(relay.stats().rooms).toBe(0);
  });
});

describe("relay rooms", () => {
  it("reports roomSize and the existing peers in roomJoined", () => {
    const { relay, peer } = setup();
    peer(nodeA);
    const b = peer(nodeB);
    const joined = b.sink.signals().find((s) => s.kind.case === "roomJoined")!;
    if (joined.kind.case !== "roomJoined") throw new Error("unreachable");
    expect(joined.kind.value.roomSize).toBe(2);
    expect(joined.kind.value.peers.map((p) => nodeIdHex(p.nodeId))).toEqual([nodeIdHex(nodeA)]);
    expect(joined.kind.value.serverMs).toBe(1_000_000n);
    expect(relay.stats()).toEqual({ rooms: 1, members: 2, connections: 2, parked: 0 });
  });

  it("does not re-announce when the same connection joins a room it is already in", () => {
    const { relay, peer } = setup();
    const a = peer(nodeA);
    const b = peer(nodeB);
    const before = b.sink.out.length;
    relay.onMessage(a.id, join(nodeA));
    expect(a.sink.kinds().filter((k) => k === "roomJoined")).toHaveLength(1);
    expect(b.sink.out.length).toBe(before);
  });

  it("keeps the same resume token when one connection joins a second room", () => {
    const { relay, peer } = setup();
    const a = peer(nodeA);
    relay.onMessage(a.id, join(nodeA, { hash: GH2 }));
    const tokens = a.sink.signals().flatMap((s) => (s.kind.case === "roomJoined" ? [nodeIdHex(s.kind.value.resumeToken)] : []));
    expect(tokens).toHaveLength(2);
    expect(tokens[0]).toBe(tokens[1]);
    expect(relay.stats()).toMatchObject({ rooms: 2, members: 2, connections: 1 });
  });

  it("rejects the member beyond MAX_ROOM_SIZE with RoomFull and admits it after someone leaves", () => {
    const { relay, peer } = setup({ MAX_ROOM_SIZE: "2" });
    const a = peer(nodeA);
    peer(nodeB);
    const c = peer(nodeC);
    expect(c.sink.errors()).toEqual([{ code: ErrorCode.RoomFull, message: "room full" }]);
    expect(relay.stats()).toMatchObject({ rooms: 1, members: 2 });
    relay.disconnect(a.id);
    relay.onMessage(c.id, join(nodeC));
    expect(c.sink.kinds()).toContain("roomJoined");
    expect(relay.stats()).toMatchObject({ rooms: 1, members: 2 });
  });

  it("changing identity on one connection leaves the old identity's rooms first", () => {
    const { relay, peer } = setup();
    const a = peer(nodeA);
    const b = peer(nodeB);
    relay.onMessage(a.id, join(nodeC, { hash: GH2 }));
    const left = b.sink.signals().filter((s) => s.kind.case === "peerEvent" && !s.kind.value.joined);
    expect(left).toHaveLength(1);
    expect(left[0]!.kind.case === "peerEvent" && nodeIdHex(left[0]!.kind.value.node!.nodeId)).toBe(nodeIdHex(nodeA));
    expect(relay.stats()).toMatchObject({ rooms: 2, members: 2, connections: 2 });
  });

  it("roomLeave for a room not joined is a no-op; empty group hash leaves every room", () => {
    const { relay, peer } = setup();
    const a = peer(nodeA);
    relay.onMessage(a.id, join(nodeA, { hash: GH2 }));
    relay.onMessage(a.id, signal({ case: "roomLeave", value: { groupHash: new Uint8Array([7, 7, 7, 7]) } }));
    expect(relay.stats()).toMatchObject({ rooms: 2, members: 2 });
    relay.onMessage(a.id, signal({ case: "roomLeave", value: {} }));
    expect(relay.stats()).toEqual({ rooms: 0, members: 0, connections: 1, parked: 0 });
  });

  it("disconnect notifies remaining peers, removes the connection and deletes emptied rooms", () => {
    const { relay, peer } = setup();
    const a = peer(nodeA);
    const b = peer(nodeB);
    relay.disconnect(b.id);
    const ev = a.sink.signals().at(-1)!;
    expect(ev.kind.case === "peerEvent" && ev.kind.value.joined).toBe(false);
    expect(relay.stats()).toEqual({ rooms: 1, members: 1, connections: 1, parked: 1 });
    relay.disconnect(a.id);
    expect(relay.stats()).toEqual({ rooms: 0, members: 0, connections: 0, parked: 2 });
    relay.disconnect(a.id); // double disconnect is harmless
    expect(relay.stats().connections).toBe(0);
  });

  it("does not park a connection that never joined a room", () => {
    const { relay, peer } = setup();
    const a = peer();
    relay.disconnect(a.id);
    expect(relay.stats()).toEqual({ rooms: 0, members: 0, connections: 0, parked: 0 });
  });
});

describe("relay signals", () => {
  it("answers ping with pong echoing tsMs and carrying server time", () => {
    const { relay, peer, tick } = setup();
    const a = peer();
    tick(250);
    relay.onMessage(a.id, signal({ case: "ping", value: { tsMs: 42n } }));
    const pong = a.sink.signals()[0]!;
    expect(pong.kind.case).toBe("pong");
    expect(pong.kind.case === "pong" && [pong.kind.value.tsMs, pong.kind.value.serverMs]).toEqual([42n, 1_000_250n]);
  });

  it("webrtc: NotInRoom before joining, dropped for unknown peer, forwarded with from rewritten to sender", () => {
    const { relay, peer } = setup();
    const a = peer();
    relay.onMessage(a.id, rtc(nodeB));
    expect(a.sink.errors()).toEqual([{ code: ErrorCode.NotInRoom, message: "not in room" }]);

    relay.onMessage(a.id, join(nodeA));
    const b = peer(nodeB);
    const bBefore = b.sink.out.length;
    relay.onMessage(a.id, rtc(nodeC));
    expect(b.sink.out.length).toBe(bBefore);

    relay.onMessage(a.id, rtc(nodeB));
    const got = b.sink.signals().at(-1)!;
    if (got.kind.case !== "webrtc") throw new Error(`expected webrtc, got ${got.kind.case}`);
    expect(nodeIdHex(got.kind.value.from)).toBe(nodeIdHex(nodeA));
    expect(got.kind.value.payload).toEqual({ case: "offerSdp", value: "v=0" });
  });
});

describe("relay resume tokens (fake timers)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-01-01T00:00:00Z"));
  });
  afterEach(() => vi.useRealTimers());

  it("resume signal within TTL restores every room including the rendezvous flag", () => {
    const { relay, peer } = clockRelay();
    const a = peer(nodeA);
    relay.onMessage(a.id, join(nodeA, { hash: GH2, rendezvous: true }));
    const token = a.sink.resumeToken();
    relay.disconnect(a.id);
    vi.advanceTimersByTime(999);

    const a2 = peer();
    relay.onMessage(a2.id, signal({ case: "resume", value: { resumeToken: token } }));
    expect(a2.sink.kinds().filter((k) => k === "roomJoined")).toHaveLength(2);
    expect(nodeIdHex(a2.sink.resumeToken())).toBe(nodeIdHex(token));
    expect(relay.stats()).toEqual({ rooms: 2, members: 2, connections: 1, parked: 0 });

    // the restored GH2 room is still rendezvous: voice from a GH2-only peer's view is filtered
    const b = peer(nodeB, { hash: GH2, rendezvous: true });
    relay.onMessage(a2.id, envelope(nodeA));
    expect(b.sink.envelopes()).toHaveLength(0);
  });

  it("resume signal at exactly TTL fails with BadResume", () => {
    const { relay, peer } = clockRelay();
    const a = peer(nodeA);
    const token = a.sink.resumeToken();
    relay.disconnect(a.id);
    vi.advanceTimersByTime(1000);
    const a2 = peer();
    relay.onMessage(a2.id, signal({ case: "resume", value: { resumeToken: token } }));
    expect(a2.sink.errors()).toEqual([{ code: ErrorCode.BadResume, message: "resume expired" }]);
    expect(relay.stats().rooms).toBe(0);
  });

  it("resume with an unknown token fails with BadResume", () => {
    const { relay, peer } = clockRelay();
    const a = peer();
    relay.onMessage(a.id, signal({ case: "resume", value: { resumeToken: new Uint8Array(16) } }));
    expect(a.sink.errors()[0]?.code).toBe(ErrorCode.BadResume);
  });

  it("room_join carrying a live token of the same node reclaims it and consumes the parking", () => {
    const { relay, peer } = clockRelay();
    const a = peer(nodeA);
    const token = a.sink.resumeToken();
    relay.disconnect(a.id);
    vi.advanceTimersByTime(500);
    const a2 = peer(nodeA, { resume: token });
    expect(nodeIdHex(a2.sink.resumeToken())).toBe(nodeIdHex(token));
    expect(relay.stats().parked).toBe(0);
  });

  it("room_join with an expired token or another node's token gets a fresh token", () => {
    const { relay, peer } = clockRelay();
    const a = peer(nodeA);
    const token = a.sink.resumeToken();
    relay.disconnect(a.id);

    const thief = peer(nodeB, { resume: token });
    expect(nodeIdHex(thief.sink.resumeToken())).not.toBe(nodeIdHex(token));
    expect(relay.stats().parked).toBe(1); // A's parking untouched

    vi.advanceTimersByTime(1000);
    const late = peer(nodeA, { resume: token, hash: GH2 });
    expect(late.sink.resumeToken()).toHaveLength(16);
    expect(nodeIdHex(late.sink.resumeToken())).not.toBe(nodeIdHex(token));
  });

  it("sweep drops only parkings whose TTL has passed", () => {
    const { relay, peer } = clockRelay();
    relay.disconnect(peer(nodeA).id);
    vi.advanceTimersByTime(600);
    relay.disconnect(peer(nodeB).id);
    expect(relay.stats().parked).toBe(2);
    vi.advanceTimersByTime(400); // A at TTL, B at 400 ms
    relay.sweep();
    expect(relay.stats().parked).toBe(1);
    vi.advanceTimersByTime(600);
    relay.sweep();
    expect(relay.stats().parked).toBe(0);
  });
});

describe("relay envelopes", () => {
  it("rejects envelopes before joining with NotInRoom", () => {
    const { relay, peer } = setup();
    const a = peer();
    relay.onMessage(a.id, envelope(nodeA));
    expect(a.sink.errors()).toEqual([{ code: ErrorCode.NotInRoom, message: "not in room" }]);
  });

  it.each([
    ["too short", tagged(WsTag.Envelope, new Uint8Array([1, 0x11, 0, 0]))],
    ["wrong version", (() => { const e = envelope(nodeA); e[1] = 9; return e; })()],
    ["unicast flag without dst bytes", (() => { const e = envelope(nodeA); e[4] = 0x01; return e; })()],
  ])("answers a malformed envelope (%s) with BadFrame and forwards nothing", (_l, frame) => {
    const { relay, peer } = setup();
    const a = peer(nodeA);
    const b = peer(nodeB);
    relay.onMessage(a.id, frame);
    expect(a.sink.errors()).toEqual([{ code: ErrorCode.BadFrame, message: "bad envelope" }]);
    expect(b.sink.envelopes()).toHaveLength(0);
  });

  it("forwards the raw frame bytes unchanged to every other member, never the sender", () => {
    const { relay, peer } = setup();
    const a = peer(nodeA);
    const b = peer(nodeB);
    const c = peer(nodeC);
    const frame = envelope(nodeA);
    relay.onMessage(a.id, frame);
    expect(b.sink.out.at(-1)).toEqual(frame);
    expect(c.sink.out.at(-1)).toEqual(frame);
    expect(a.sink.envelopes()).toHaveLength(0);
  });

  it("delivers once to a peer shared through two rooms", () => {
    const { relay, peer } = setup();
    const a = peer(nodeA);
    relay.onMessage(a.id, join(nodeA, { hash: GH2 }));
    const b = peer(nodeB);
    relay.onMessage(b.id, join(nodeB, { hash: GH2 }));
    relay.onMessage(a.id, envelope(nodeA));
    expect(b.sink.envelopes()).toHaveLength(1);
  });

  it.each([
    ["VoiceRouted", FrameType.VoiceRouted, 0],
    ["VoiceFlood", FrameType.VoiceFlood, 0],
    ["Message", FrameType.Message, 0],
    ["Fragment", FrameType.Fragment, 0],
    ["Hello", FrameType.Hello, 1],
    ["Handshake", FrameType.Handshake, 1],
    ["Control", FrameType.Control, 1],
  ])("rendezvous room forwards %s broadcast %i time(s)", (_n, ftype, expected) => {
    const { relay, peer } = setup();
    const a = peer(nodeA, { rendezvous: true });
    const b = peer(nodeB, { rendezvous: true });
    relay.onMessage(a.id, envelope(nodeA, { ftype }));
    expect(b.sink.envelopes()).toHaveLength(expected);
  });

  it("drops frames over RATE_LIMIT_FPS, warns once per RATE_LIMIT_FPS frames, refills with time", () => {
    const { relay, peer, tick } = setup();
    const a = peer(nodeA);
    const b = peer(nodeB);
    for (let i = 0; i < 15; i++) relay.onMessage(a.id, envelope(nodeA));
    expect(b.sink.envelopes()).toHaveLength(5);
    expect(a.sink.errors()).toEqual([
      { code: ErrorCode.RateLimited, message: "rate limited" },
      { code: ErrorCode.RateLimited, message: "rate limited" },
    ]);
    tick(400); // 2 tokens back
    for (let i = 0; i < 3; i++) relay.onMessage(a.id, envelope(nodeA));
    expect(b.sink.envelopes()).toHaveLength(7);
    tick(10_000); // capped at RATE_LIMIT_FPS, not 50
    for (let i = 0; i < 10; i++) relay.onMessage(a.id, envelope(nodeA));
    expect(b.sink.envelopes()).toHaveLength(12);
  });
});
