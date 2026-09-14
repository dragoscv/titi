import { create, fromBinary, toBinary } from "@bufbuild/protobuf";
import { Flags, PROTOCOL_VERSION, SignalSchema, WsTag, type Signal } from "@titi/protocol";
import { describe, expect, it } from "vitest";
import { loadConfig } from "../src/config.ts";
import { Relay, tagged, type Sink } from "../src/relay.ts";

class FakeSink implements Sink {
  out: Uint8Array[] = [];
  closed: { code: number; reason: string } | null = null;
  send(b: Uint8Array) {
    this.out.push(b);
  }
  close(code: number, reason: string) {
    this.closed = { code, reason };
  }
  signals(): Signal[] {
    return this.out.filter((b) => b[0] === WsTag.Signal).map((b) => fromBinary(SignalSchema, b.subarray(1)));
  }
  envelopes(): Uint8Array[] {
    return this.out.filter((b) => b[0] === WsTag.Envelope).map((b) => b.subarray(1));
  }
}

const GH = new Uint8Array([1, 2, 3, 4]);
const nodeA = new Uint8Array([0xa, 1, 2, 3, 4, 5, 6, 7]);
const nodeB = new Uint8Array([0xb, 1, 2, 3, 4, 5, 6, 7]);
const nodeC = new Uint8Array([0xc, 1, 2, 3, 4, 5, 6, 7]);

function join(node: Uint8Array, resume?: Uint8Array, hash: Uint8Array = GH, rendezvous = false): Uint8Array {
  const s = create(SignalSchema, {
    kind: {
      case: "roomJoin",
      value: { groupHash: hash, node: { nodeId: node, displayName: "n" }, resumeToken: resume ?? new Uint8Array(), rendezvous },
    },
  });
  return tagged(WsTag.Signal, toBinary(SignalSchema, s));
}

function envelope(src: Uint8Array, dst?: Uint8Array, flags = 0, ftype = 0x11): Uint8Array {
  const hdr = dst ? 24 : 16;
  const b = new Uint8Array(hdr + 3);
  b[0] = PROTOCOL_VERSION;
  b[1] = ftype; // default VoiceFlood
  b[2] = 0x33;
  b[3] = flags | (dst ? Flags.UNICAST : 0);
  b.set(src, 8);
  if (dst) b.set(dst, 16);
  b.set([9, 9, 9], hdr);
  return tagged(WsTag.Envelope, b);
}

function setup() {
  let t = 1_000_000;
  const cfg = loadConfig({ RATE_LIMIT_FPS: "5", RESUME_TTL_MS: "1000", MAX_ROOM_SIZE: "2" });
  const relay = new Relay(cfg, () => t);
  return { relay, tick: (ms: number) => (t += ms) };
}

describe("relay", () => {
  it("multi-room: rendezvous rooms pass hello but not voice; leaving one room keeps the other", () => {
    const { relay } = setup();
    const RDV = new Uint8Array([9, 9, 9, 9]);
    const a = new FakeSink();
    const b = new FakeSink();
    const ia = relay.connect(a);
    const ib = relay.connect(b);
    relay.onMessage(ia, join(nodeA)); // A: group room
    relay.onMessage(ia, join(nodeA, undefined, RDV, true)); // A: + rendezvous
    relay.onMessage(ib, join(nodeB, undefined, RDV, true)); // B: rendezvous only (joiner typing a code)
    expect(relay.stats()).toMatchObject({ rooms: 2, members: 3 });
    relay.onMessage(ia, envelope(nodeA)); // voice → not forwarded through rendezvous
    expect(b.envelopes().length).toBe(0);
    relay.onMessage(ia, envelope(nodeA, undefined, 0, 0x01)); // Hello → forwarded
    expect(b.envelopes().length).toBe(1);
    relay.onMessage(ib, envelope(nodeB, nodeA, 0, 0x04)); // Handshake unicast via shared room
    expect(a.envelopes().length).toBe(1);
    // A leaves rendezvous only
    const leave = create(SignalSchema, { kind: { case: "roomLeave", value: { groupHash: RDV } } });
    relay.onMessage(ia, tagged(WsTag.Signal, toBinary(SignalSchema, leave)));
    expect(relay.stats()).toMatchObject({ rooms: 2, members: 2 });
    relay.onMessage(ia, envelope(nodeA, undefined, 0, 0x01));
    expect(b.envelopes().length).toBe(1); // no longer shared
  });

  it("joins, announces peers, broadcasts and unicasts envelopes", () => {
    const { relay } = setup();
    const a = new FakeSink();
    const b = new FakeSink();
    const ia = relay.connect(a);
    const ib = relay.connect(b);
    relay.onMessage(ia, join(nodeA));
    relay.onMessage(ib, join(nodeB));

    const joinedA = a.signals().find((s) => s.kind.case === "roomJoined");
    expect(joinedA).toBeTruthy();
    expect(a.signals().some((s) => s.kind.case === "peerEvent")).toBe(true); // B joined
    const joinedB = b.signals().find((s) => s.kind.case === "roomJoined")!;
    expect(joinedB.kind.case === "roomJoined" && joinedB.kind.value.peers.length).toBe(1);

    relay.onMessage(ia, envelope(nodeA));
    expect(b.envelopes().length).toBe(1);
    expect(a.envelopes().length).toBe(0); // never echoed to sender

    relay.onMessage(ib, envelope(nodeB, nodeA));
    expect(a.envelopes().length).toBe(1);
    relay.onMessage(ib, envelope(nodeB, nodeC)); // unknown dst → dropped
    expect(a.envelopes().length).toBe(1);
  });

  it("rejects spoofed src unless RELAYED", () => {
    const { relay } = setup();
    const a = new FakeSink();
    const b = new FakeSink();
    const ia = relay.connect(a);
    const ib = relay.connect(b);
    relay.onMessage(ia, join(nodeA));
    relay.onMessage(ib, join(nodeB));
    relay.onMessage(ia, envelope(nodeC));
    expect(b.envelopes().length).toBe(0);
    expect(a.signals().some((s) => s.kind.case === "error")).toBe(true);
    relay.onMessage(ia, envelope(nodeC, undefined, Flags.RELAYED));
    expect(b.envelopes().length).toBe(1);
  });

  it("enforces room size and rate limit", () => {
    const { relay, tick } = setup();
    const sinks = [new FakeSink(), new FakeSink(), new FakeSink()];
    const ids = sinks.map((s) => relay.connect(s));
    relay.onMessage(ids[0]!, join(nodeA));
    relay.onMessage(ids[1]!, join(nodeB));
    relay.onMessage(ids[2]!, join(nodeC));
    const err = sinks[2]!.signals().find((s) => s.kind.case === "error");
    expect(err && err.kind.case === "error" && err.kind.value.code).toBe(3);

    for (let i = 0; i < 8; i++) relay.onMessage(ids[0]!, envelope(nodeA));
    expect(sinks[1]!.envelopes().length).toBe(5);
    tick(1000);
    relay.onMessage(ids[0]!, envelope(nodeA));
    expect(sinks[1]!.envelopes().length).toBe(6);
  });

  it("resumes with token after disconnect, replaces stale connection", () => {
    const { relay, tick } = setup();
    const a = new FakeSink();
    const ia = relay.connect(a);
    relay.onMessage(ia, join(nodeA));
    const joined = a.signals().find((s) => s.kind.case === "roomJoined")!;
    const token = joined.kind.case === "roomJoined" ? joined.kind.value.resumeToken : new Uint8Array();
    expect(token.length).toBe(16);
    relay.disconnect(ia);
    expect(relay.stats().parked).toBe(1);

    const a2 = new FakeSink();
    const ia2 = relay.connect(a2);
    const resume = create(SignalSchema, { kind: { case: "resume", value: { resumeToken: token } } });
    relay.onMessage(ia2, tagged(WsTag.Signal, toBinary(SignalSchema, resume)));
    expect(a2.signals().some((s) => s.kind.case === "roomJoined")).toBe(true);
    expect(relay.stats()).toMatchObject({ rooms: 1, members: 1, parked: 0 });

    // same node joining from a third socket replaces the second
    const a3 = new FakeSink();
    relay.onMessage(relay.connect(a3), join(nodeA));
    expect(a2.closed?.code).toBe(4000);
    expect(relay.stats().members).toBe(1);

    // expired token
    relay.disconnect(relay.connect(new FakeSink())); // no-op path
    tick(5000);
    relay.sweep();
    const a4 = new FakeSink();
    const ia4 = relay.connect(a4);
    relay.onMessage(ia4, tagged(WsTag.Signal, toBinary(SignalSchema, resume)));
    expect(a4.signals().some((s) => s.kind.case === "error")).toBe(true);
  });
});
