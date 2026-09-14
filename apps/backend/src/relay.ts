// Forward-only relay of Titi envelopes between members of a room (ADR-0005).
// The relay never sees group keys: rooms are keyed by the 4-byte group hash
// and every payload is opaque group-AEAD ciphertext. This module is pure
// (no sockets) so it can be unit-tested; ws.ts adapts it to WebSockets.

import { create, fromBinary, toBinary, type MessageInitShape } from "@bufbuild/protobuf";
import {
  bytesEqual,
  decodeHeader,
  Flags,
  nodeIdHex,
  NodeRefSchema,
  SignalSchema,
  WsTag,
  type NodeRef,
  type Signal,
} from "@titi/protocol";
import { randomBytes } from "node:crypto";
import type { Config } from "./config.ts";
import { log } from "./log.ts";

export interface Sink {
  send(bytes: Uint8Array): void;
  close(code: number, reason: string): void;
}

interface Member {
  id: number;
  node: NodeRef;
  nodeHex: string;
  sink: Sink;
  room: Room | null;
  resumeToken: Uint8Array;
  // token-bucket rate limit
  tokens: number;
  lastRefill: number;
  frames: number;
}

interface Room {
  hash: string;
  members: Map<number, Member>;
}

interface Parked {
  node: NodeRef;
  roomHash: string;
  expiresAt: number;
}

export const ErrorCode = {
  BadFrame: 1,
  NotInRoom: 2,
  RoomFull: 3,
  RateLimited: 4,
  BadResume: 5,
  TooLarge: 6,
} as const;

export class Relay {
  private rooms = new Map<string, Room>();
  private members = new Map<number, Member>();
  private parked = new Map<string, Parked>(); // resume token hex → parked member
  private nextId = 1;

  constructor(
    private readonly cfg: Config,
    private readonly now: () => number = Date.now,
  ) {}

  /** Register a new connection. Returns a connection id used for subsequent calls. */
  connect(sink: Sink): number {
    const id = this.nextId++;
    this.members.set(id, {
      id,
      node: create(NodeRefSchema),
      nodeHex: "",
      sink,
      room: null,
      resumeToken: new Uint8Array(),
      tokens: this.cfg.RATE_LIMIT_FPS,
      lastRefill: this.now(),
      frames: 0,
    });
    return id;
  }

  disconnect(id: number) {
    const m = this.members.get(id);
    if (!m) return;
    this.members.delete(id);
    if (m.room) {
      this.leaveRoom(m, /*park*/ true);
    }
  }

  /** Incoming binary WebSocket message. */
  onMessage(id: number, data: Uint8Array) {
    const m = this.members.get(id);
    if (!m) return;
    if (data.length === 0) return;
    if (data.length > this.cfg.MAX_FRAME_BYTES + 1) {
      this.sendError(m, ErrorCode.TooLarge, "frame too large");
      return;
    }
    const tag = data[0];
    const body = data.subarray(1);
    if (tag === WsTag.Signal) {
      let sig: Signal;
      try {
        sig = fromBinary(SignalSchema, body);
      } catch {
        this.sendError(m, ErrorCode.BadFrame, "bad signal");
        return;
      }
      this.onSignal(m, sig);
    } else if (tag === WsTag.Envelope) {
      this.onEnvelope(m, body, data);
    } else {
      this.sendError(m, ErrorCode.BadFrame, "unknown tag");
    }
  }

  /** Expire parked resume tokens. Call periodically. */
  sweep() {
    const t = this.now();
    for (const [k, p] of this.parked) if (p.expiresAt <= t) this.parked.delete(k);
  }

  stats() {
    let members = 0;
    for (const r of this.rooms.values()) members += r.members.size;
    return { rooms: this.rooms.size, members, connections: this.members.size, parked: this.parked.size };
  }

  // ---- signals ---------------------------------------------------------

  private onSignal(m: Member, sig: Signal) {
    const k = sig.kind;
    switch (k.case) {
      case "roomJoin": {
        const j = k.value;
        if (j.groupHash.length !== 4 || !j.node || j.node.nodeId.length !== 8) {
          this.sendError(m, ErrorCode.BadFrame, "bad room_join");
          return;
        }
        if (m.room) this.leaveRoom(m, false);
        m.node = j.node;
        m.nodeHex = nodeIdHex(j.node.nodeId);
        // resume: same node re-joining reclaims its token, else a fresh one
        const parkedKey = j.resumeToken.length ? nodeIdHex(j.resumeToken) : null;
        const parked = parkedKey ? this.parked.get(parkedKey) : undefined;
        if (parked && parked.expiresAt > this.now() && nodeIdHex(parked.node.nodeId) === m.nodeHex) {
          this.parked.delete(parkedKey!);
          m.resumeToken = j.resumeToken;
        } else {
          m.resumeToken = randomBytes(16);
        }
        this.joinRoom(m, nodeIdHex(j.groupHash));
        return;
      }
      case "resume": {
        const key = nodeIdHex(k.value.resumeToken);
        const p = this.parked.get(key);
        if (!p || p.expiresAt <= this.now()) {
          this.sendError(m, ErrorCode.BadResume, "resume expired");
          return;
        }
        this.parked.delete(key);
        m.node = p.node;
        m.nodeHex = nodeIdHex(p.node.nodeId);
        m.resumeToken = k.value.resumeToken;
        this.joinRoom(m, p.roomHash);
        return;
      }
      case "roomLeave":
        if (m.room) this.leaveRoom(m, false);
        return;
      case "ping":
        this.sendSignal(m, { case: "pong", value: { tsMs: k.value.tsMs, serverMs: BigInt(this.now()) } });
        return;
      case "webrtc": {
        // pass-through to a specific peer in the same room (web LAN bootstrap)
        if (!m.room) return this.sendError(m, ErrorCode.NotInRoom, "not in room");
        const to = nodeIdHex(k.value.to);
        for (const peer of m.room.members.values()) {
          if (peer.nodeHex === to) {
            const fwd = create(SignalSchema, { kind: { case: "webrtc", value: { ...k.value, from: m.node.nodeId } } });
            peer.sink.send(tagged(WsTag.Signal, toBinary(SignalSchema, fwd)));
            return;
          }
        }
        return;
      }
      default:
        return;
    }
  }

  private joinRoom(m: Member, hash: string) {
    let room = this.rooms.get(hash);
    if (!room) {
      room = { hash, members: new Map() };
      this.rooms.set(hash, room);
    }
    // a reconnecting node replaces its stale connection
    for (const other of room.members.values()) {
      if (other.nodeHex === m.nodeHex && other.id !== m.id) {
        room.members.delete(other.id);
        other.room = null;
        other.sink.close(4000, "replaced");
        this.members.delete(other.id);
      }
    }
    if (room.members.size >= this.cfg.MAX_ROOM_SIZE) {
      this.sendError(m, ErrorCode.RoomFull, "room full");
      if (room.members.size === 0) this.rooms.delete(hash);
      return;
    }
    const peers = [...room.members.values()].map((p) => p.node);
    room.members.set(m.id, m);
    m.room = room;
    this.sendSignal(m, {
      case: "roomJoined",
      value: { resumeToken: m.resumeToken, peers, roomSize: room.members.size, serverMs: BigInt(this.now()) },
    });
    const ev = create(SignalSchema, { kind: { case: "peerEvent", value: { node: m.node, joined: true } } });
    const evBytes = tagged(WsTag.Signal, toBinary(SignalSchema, ev));
    for (const p of room.members.values()) if (p.id !== m.id) p.sink.send(evBytes);
    log.info("room.join", { room: hash, node: m.nodeHex, size: room.members.size });
  }

  private leaveRoom(m: Member, park: boolean) {
    const room = m.room;
    if (!room) return;
    room.members.delete(m.id);
    m.room = null;
    if (park && m.resumeToken.length) {
      this.parked.set(nodeIdHex(m.resumeToken), {
        node: m.node,
        roomHash: room.hash,
        expiresAt: this.now() + this.cfg.RESUME_TTL_MS,
      });
    }
    const ev = create(SignalSchema, { kind: { case: "peerEvent", value: { node: m.node, joined: false } } });
    const evBytes = tagged(WsTag.Signal, toBinary(SignalSchema, ev));
    for (const p of room.members.values()) p.sink.send(evBytes);
    if (room.members.size === 0) this.rooms.delete(room.hash);
    log.info("room.leave", { room: room.hash, node: m.nodeHex, size: room.members.size, parked: park });
  }

  // ---- envelopes -------------------------------------------------------

  private onEnvelope(m: Member, env: Uint8Array, raw: Uint8Array) {
    if (!m.room) return this.sendError(m, ErrorCode.NotInRoom, "not in room");
    if (!this.takeToken(m)) {
      // silently drop voice under pressure; tell the client once per second
      if (m.frames % this.cfg.RATE_LIMIT_FPS === 0) this.sendError(m, ErrorCode.RateLimited, "rate limited");
      return;
    }
    const h = decodeHeader(env);
    if (!h) return this.sendError(m, ErrorCode.BadFrame, "bad envelope");
    // src must be the authenticated node of this connection — no spoofing
    if (!bytesEqual(h.src, m.node.nodeId)) {
      // relayed frames from a mesh neighbour are legitimate: the phone mirrors
      // its LAN mesh into the room so online members hear everyone.
      if ((h.flags & Flags.RELAYED) === 0) return this.sendError(m, ErrorCode.BadFrame, "src mismatch");
    }
    if (h.dst) {
      const to = nodeIdHex(h.dst);
      for (const p of m.room.members.values()) {
        if (p.nodeHex === to) {
          p.sink.send(raw);
          return;
        }
      }
      return; // unknown dst in this room: drop
    }
    for (const p of m.room.members.values()) if (p.id !== m.id) p.sink.send(raw);
  }

  private takeToken(m: Member): boolean {
    const t = this.now();
    const elapsed = (t - m.lastRefill) / 1000;
    if (elapsed > 0) {
      m.tokens = Math.min(this.cfg.RATE_LIMIT_FPS, m.tokens + elapsed * this.cfg.RATE_LIMIT_FPS);
      m.lastRefill = t;
    }
    m.frames++;
    if (m.tokens < 1) return false;
    m.tokens -= 1;
    return true;
  }

  // ---- helpers ---------------------------------------------------------

  private sendSignal(m: Member, kind: MessageInitShape<typeof SignalSchema>["kind"]) {
    const sig = create(SignalSchema, { kind });
    m.sink.send(tagged(WsTag.Signal, toBinary(SignalSchema, sig)));
  }

  private sendError(m: Member, code: number, message: string) {
    this.sendSignal(m, { case: "error", value: { code, message } });
  }
}

export function tagged(tag: number, body: Uint8Array): Uint8Array {
  const out = new Uint8Array(body.length + 1);
  out[0] = tag;
  out.set(body, 1);
  return out;
}
