// WebSocket transport to the Cloud Run relay (ADR-0005). Mirrors
// android RelayTransport: room = 4-byte group hash, tag 0x00 signal / 0x01 envelope.
import { create, fromBinary, toBinary } from "@bufbuild/protobuf";
import { SignalSchema, WsTag } from "@titi/protocol";

export interface RelayEvents {
  linkUp(): void;
  linkDown(): void;
  peerSeen(token: string): void;
  peerLost(token: string): void;
  frame(token: string, bytes: Uint8Array): void;
  webrtc?(from: string, payload: { offerSdp?: string; answerSdp?: string; ice?: string }): void;
}

const hex = (b: Uint8Array) => Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
const hexToBytes = (h: string) => Uint8Array.from(h.match(/.{2}/g) ?? [], (b) => parseInt(b, 16));

export class RelayTransport {
  private ws: WebSocket | null = null;
  /** hex(hash) → rendezvous flag */
  private rooms = new Map<string, boolean>();
  private joinedRooms = new Set<string>();
  private resumeToken = new Uint8Array();
  private attempt = 0;
  private timer: number | null = null;
  private joined = false;
  private enabled = false;
  private peers = new Set<string>();

  constructor(
    private url: () => string,
    private nodeId: () => Uint8Array,
    private displayName: () => string,
    private hue: () => number,
    private ev: RelayEvents,
  ) {}

  start() {
    this.enabled = true;
    this.connect();
  }
  stop() {
    this.enabled = false;
    if (this.timer) clearTimeout(this.timer);
    this.ws?.close(1000, "bye");
    this.ws = null;
    if (this.joined) { this.joined = false; this.ev.linkDown(); }
  }

  /** Desired room set: group rooms (voice) + rendezvous rooms (discovery only). */
  setRooms(rooms: { hash: Uint8Array; rendezvous: boolean }[]) {
    const want = new Map(rooms.map((r) => [hex(r.hash), r.rendezvous] as const));
    const same = want.size === this.rooms.size && [...want].every(([k, v]) => this.rooms.get(k) === v);
    if (same) return;
    // leave rooms no longer wanted
    if (this.ws?.readyState === WebSocket.OPEN) {
      for (const k of this.rooms.keys()) if (!want.has(k)) this.sendLeave(hexToBytes(k));
    }
    this.rooms = want;
    if (this.ws?.readyState === WebSocket.OPEN) this.sendJoin();
  }

  send(bytes: Uint8Array) {
    if (!this.joined || this.ws?.readyState !== WebSocket.OPEN) return;
    const out = new Uint8Array(bytes.length + 1);
    out[0] = WsTag.Envelope;
    out.set(bytes, 1);
    this.ws.send(out);
  }

  sendWebRtc(to: Uint8Array, payload: { offerSdp?: string; answerSdp?: string; ice?: string }) {
    if (this.ws?.readyState !== WebSocket.OPEN) return;
    const p = payload.offerSdp !== undefined ? { case: "offerSdp" as const, value: payload.offerSdp }
      : payload.answerSdp !== undefined ? { case: "answerSdp" as const, value: payload.answerSdp }
      : { case: "iceCandidate" as const, value: payload.ice ?? "" };
    const sig = create(SignalSchema, { kind: { case: "webrtc", value: { to, from: this.nodeId(), payload: p } } });
    this.sendSignal(toBinary(SignalSchema, sig));
  }

  private connect() {
    if (!this.enabled) return;
    const ws = new WebSocket(this.url());
    ws.binaryType = "arraybuffer";
    ws.onopen = () => { this.attempt = 0; this.joinedRooms.clear(); this.sendJoin(); };
    ws.onmessage = (e) => this.onMessage(new Uint8Array(e.data as ArrayBuffer));
    ws.onclose = () => this.onClosed();
    ws.onerror = () => { /* onclose follows */ };
    this.ws = ws;
  }

  private onClosed() {
    this.ws = null;
    if (this.joined) {
      this.joined = false;
      for (const p of this.peers) this.ev.peerLost(p);
      this.peers.clear();
      this.ev.linkDown();
    }
    if (this.enabled) {
      this.attempt++;
      const backoff = Math.min(30_000, 500 * 2 ** Math.min(this.attempt, 6));
      this.timer = window.setTimeout(() => this.connect(), backoff);
    }
  }

  private sendSignal(body: Uint8Array) {
    const out = new Uint8Array(body.length + 1);
    out[0] = WsTag.Signal;
    out.set(body, 1);
    this.ws?.send(out);
  }

  private sendJoin() {
    for (const [k, rendezvous] of this.rooms) {
      if (this.joinedRooms.has(k)) continue;
      const sig = create(SignalSchema, {
        kind: {
          case: "roomJoin",
          value: { groupHash: hexToBytes(k), node: { nodeId: this.nodeId(), displayName: this.displayName(), avatarHue: this.hue() }, resumeToken: this.resumeToken, rendezvous },
        },
      });
      this.sendSignal(toBinary(SignalSchema, sig));
      this.joinedRooms.add(k);
    }
  }

  private sendLeave(hash: Uint8Array) {
    this.joinedRooms.delete(hex(hash));
    const sig = create(SignalSchema, { kind: { case: "roomLeave", value: { groupHash: hash } } });
    this.sendSignal(toBinary(SignalSchema, sig));
  }

  private onMessage(b: Uint8Array) {
    if (b.length === 0) return;
    const body = b.subarray(1);
    if (b[0] === WsTag.Envelope) {
      if (body.length < 16) return;
      const src = hex(body.subarray(8, 16));
      if (!this.peers.has(src)) { this.peers.add(src); this.ev.peerSeen(src); }
      this.ev.frame(src, body);
      return;
    }
    if (b[0] !== WsTag.Signal) return;
    let sig;
    try { sig = fromBinary(SignalSchema, body); } catch { return; }
    const k = sig.kind;
    switch (k.case) {
      case "roomJoined": {
        this.resumeToken = new Uint8Array(k.value.resumeToken);
        for (const p of k.value.peers) { const id = hex(p.nodeId); if (!this.peers.has(id)) { this.peers.add(id); this.ev.peerSeen(id); } }
        if (!this.joined) { this.joined = true; this.ev.linkUp(); }
        break;
      }
      case "peerEvent": {
        const id = k.value.node ? hex(k.value.node.nodeId) : null;
        if (!id) break;
        if (k.value.joined) { if (!this.peers.has(id)) { this.peers.add(id); this.ev.peerSeen(id); } }
        else if (this.peers.delete(id)) this.ev.peerLost(id);
        break;
      }
      case "webrtc": {
        const p = k.value.payload;
        this.ev.webrtc?.(hex(k.value.from), {
          offerSdp: p.case === "offerSdp" ? p.value : undefined,
          answerSdp: p.case === "answerSdp" ? p.value : undefined,
          ice: p.case === "iceCandidate" ? p.value : undefined,
        });
        break;
      }
      case "error":
        if (k.value.code === 5) { this.resumeToken = new Uint8Array(); this.joinedRooms.clear(); this.sendJoin(); }
        console.warn("relay error", k.value.code, k.value.message);
        break;
      default:
        break;
    }
  }
}
