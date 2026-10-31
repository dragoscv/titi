// App Store review "demo group" bot (runs inside the relay process).
//
// A reviewer with one iPhone joins a fixed group by link and sees voice and
// chat working: the bot is a regular group member driven by the same Rust
// engine as the web app (wasm, nodejs target), connected to the relay through
// an in-process Sink instead of a WebSocket. It never sees anything the relay
// would not: group keys live only inside its own engine, like any member.
//
// Behaviour: parrots the last voice turn back (700 ms after the floor goes
// idle, max 30 s), reacts 👍 + quotes every text message, "/delete" deletes
// its last message, "/edit" shows an edit. Rate-limited per sender and
// globally so the bot cannot be used as an amplifier.

import { create, fromBinary, toBinary } from "@bufbuild/protobuf";
import { SignalSchema, WsTag, type Signal } from "@titi/protocol";
import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { log } from "./log.ts";
import { tagged, type Relay, type Sink } from "./relay.ts";

/** Subset of the wasm-bindgen `WasmEngine` (core/titi-wasm) the bot uses. */
export interface WasmEngineLike {
  node_id(): Uint8Array;
  identity_seed(): Uint8Array;
  restore_groups(data: Uint8Array, now: number): boolean;
  groups_json(): string;
  set_active_group(group: Uint8Array): void;
  is_talking(): boolean;
  on_link_up(link: number, cls: number, now: number): string;
  on_link_stats(link: number, bps: number, rtt: number, loss: number, now: number): string;
  on_peer_seen(link: number, token: string, now: number): string;
  on_peer_lost(link: number, token: string, now: number): string;
  on_frame(link: number, token: string, bytes: Uint8Array, now: number): string;
  tick(now: number): string;
  ptt_down(prio: number, now: number): string;
  ptt_up(now: number): string;
  on_opus_in(packet: Uint8Array, now: number): string;
  send_text_v2(group: Uint8Array, text: string, replyTo: Uint8Array, mentions: Uint8Array, all: boolean, now: number): string;
  react(group: Uint8Array, target: Uint8Array, emoji: string, now: number): string;
  edit_message(group: Uint8Array, target: Uint8Array, text: string, now: number): string;
  delete_message(group: Uint8Array, target: Uint8Array, now: number): string;
}

export interface WasmModule {
  WasmEngine: new (seed: Uint8Array, name: string, hue: number, rng: bigint) => WasmEngineLike;
  group_hash(group: Uint8Array): Uint8Array;
}

/**
 * Load packages/core-wasm/pkg-node (CommonJS, wasm read from disk next to it).
 * Resolved by path, not as a workspace dependency, so it works unchanged in
 * dev (tsx/vitest from src/), in the built dist/ inside the monorepo (both are
 * three levels below the repo root) and in the Docker image, where the
 * Dockerfile copies pkg-node to /app/wasm next to /app/dist.
 */
export function loadWasm(): WasmModule {
  const candidates = ["../../../packages/core-wasm/pkg-node/titi_wasm.js", "../wasm/titi_wasm.js"].map((p) => fileURLToPath(new URL(p, import.meta.url)));
  const file = candidates.find((p) => existsSync(p));
  if (!file) throw new Error("titi wasm (pkg-node) not found");
  return createRequire(import.meta.url)(file) as WasmModule;
}

/** LinkId / LinkClass of the relay link (packages/app-ui/src/types.ts LinkIds.INTERNET). */
const LINK = 8;
const CLASS_INTERNET = 8;
const TICK_MS = 20;
export const BOT_NAME = "Titi Demo";
const BOT_HUE = 200;
/** 30 s of 20 ms packets. */
export const PARROT_MAX_PACKETS = 1500;
export const PARROT_IDLE_MS = 700;
/** Replay even without a floorIdle (lost frame): this long after the last packet. */
const PARROT_SILENCE_MS = 2_000;
/** Give up if the floor is not granted this long after ptt_down. */
const PARROT_GRANT_TIMEOUT_MS = 2_000;
const EDIT_DELAY_MS = 1_500;
const QUOTE_CHARS = 80;

type MsgBody = { kind: string; text?: string };
type UiEvent =
  | { type: "message"; group: string; from: string; msg_uuid: string; body: MsgBody }
  | { type: "floorTaken"; group: string; holder: string }
  | { type: "floorIdle"; group: string }
  | { type: string };
type Action =
  | { t: "send"; link: number; bytes: number[] }
  | { t: "playPacket"; talker: string; packet: number[] }
  | { t: "ui"; event: UiEvent }
  | { t: string };

export const hex = (b: Uint8Array) => Buffer.from(b).toString("hex");
export const unhex = (h: string) => new Uint8Array(Buffer.from(h, "hex"));

// ---- rate limit -----------------------------------------------------------

interface Bucket { tokens: number; at: number }

/** Token buckets: `perKey` actions per window per sender and `global` in total. */
export class RateLimiter {
  private keys = new Map<string, Bucket>();
  private all: Bucket | null = null;
  constructor(private readonly perKey = 10, private readonly global = 60, private readonly windowMs = 60_000) {}

  take(key: string, now: number): boolean {
    const k = this.refill(this.keys.get(key) ?? { tokens: this.perKey, at: now }, this.perKey, now);
    const g = this.refill(this.all ?? { tokens: this.global, at: now }, this.global, now);
    this.all = g;
    this.keys.set(key, k);
    if (this.keys.size > 1_000) for (const [id, b] of this.keys) if (b.tokens >= this.perKey) this.keys.delete(id);
    if (k.tokens < 1 || g.tokens < 1) return false;
    k.tokens -= 1;
    g.tokens -= 1;
    return true;
  }

  private refill(b: Bucket, cap: number, now: number): Bucket {
    b.tokens = Math.min(cap, b.tokens + ((now - b.at) * cap) / this.windowMs);
    b.at = now;
    return b;
  }
}

// ---- voice parrot ---------------------------------------------------------

export type ParrotStep =
  | { t: "none" }
  | { t: "down"; talker: string }
  | { t: "wait" }
  | { t: "packet"; packet: Uint8Array }
  | { t: "up" };

/** Records one talker's turn and schedules its replay; pure (clock passed in). */
export class Parrot {
  private buf: Uint8Array[] = [];
  private talker = "";
  private lastAt = 0;
  private idleAt: number | null = null;
  private playing: Uint8Array[] | null = null;
  private downAt = 0;
  private granted = false;

  get recorded() { return this.buf.length; }
  get busy() { return this.playing !== null; }

  collect(talker: string, packet: Uint8Array, now: number) {
    if (this.playing || packet.length === 0) return;
    if (this.buf.length === 0) this.talker = talker;
    if (talker !== this.talker) return;
    if (this.buf.length < PARROT_MAX_PACKETS) this.buf.push(packet);
    this.lastAt = now;
  }
  floorTaken() { if (!this.playing) this.idleAt = null; }
  floorIdle(now: number) { if (!this.playing && this.buf.length) this.idleAt = now; }
  cancel() { this.buf = []; this.playing = null; this.idleAt = null; }

  step(now: number, talking: boolean): ParrotStep {
    if (this.playing) {
      if (!talking) {
        // waiting for the grant, or pre-empted mid-replay
        if (this.granted || now - this.downAt > PARROT_GRANT_TIMEOUT_MS) { this.playing = null; return { t: "up" }; }
        return { t: "wait" };
      }
      this.granted = true;
      const p = this.playing.shift();
      if (p) return { t: "packet", packet: p };
      this.playing = null;
      return { t: "up" };
    }
    if (this.buf.length === 0) return { t: "none" };
    const idle = this.idleAt !== null && now - Math.max(this.idleAt, this.lastAt) >= PARROT_IDLE_MS;
    if (!idle && now - this.lastAt < PARROT_SILENCE_MS) return { t: "none" };
    this.playing = this.buf;
    this.buf = [];
    this.idleAt = null;
    this.downAt = now;
    this.granted = false;
    return { t: "down", talker: this.talker };
  }
}

// ---- in-process relay member ----------------------------------------------

/**
 * One engine attached to the relay through an in-process Sink. Mirrors
 * packages/app-ui/src/web/relay.ts + host.ts (signals → link/peer events,
 * envelopes → on_frame, Send actions → relay). Relay callbacks are queued and
 * drained after the current engine call so engine calls never nest.
 */
export class InProcessPeer {
  readonly node: Uint8Array;
  readonly nodeHex: string;
  private id: number;
  private inbox: Uint8Array[] = [];
  private busy = false;
  private linkUp = false;
  private peers = new Set<string>();

  constructor(
    private readonly relay: Relay,
    readonly eng: WasmEngineLike,
    private readonly opts: { name: string; hue: number; now: () => number; onAction: (a: Action) => void },
  ) {
    this.node = eng.node_id();
    this.nodeHex = hex(this.node);
    const sink: Sink = {
      send: (b) => { this.inbox.push(b); this.guarded(() => {}); },
      close: (code, reason) => log.warn("demo_bot.closed", { code, reason }),
    };
    this.id = relay.connect(sink);
  }

  join(hashes: Uint8Array[]) {
    for (const groupHash of hashes) {
      const sig = create(SignalSchema, {
        kind: { case: "roomJoin", value: { groupHash, node: { nodeId: this.node, displayName: this.opts.name, avatarHue: this.opts.hue }, rendezvous: false } },
      });
      this.relay.onMessage(this.id, tagged(WsTag.Signal, toBinary(SignalSchema, sig)));
    }
  }

  /** Run an engine call and dispatch its actions. */
  run(f: (e: WasmEngineLike) => string) {
    this.guarded(() => this.call(f));
  }

  disconnect() { this.relay.disconnect(this.id); }

  private guarded(f: () => void) {
    if (this.busy) return f();
    this.busy = true;
    try {
      f();
      for (let b = this.inbox.shift(); b; b = this.inbox.shift()) this.onRelay(b);
    } finally {
      this.busy = false;
    }
  }

  private call(f: (e: WasmEngineLike) => string) {
    let acts: Action[];
    try {
      acts = JSON.parse(f(this.eng)) as Action[];
    } catch (e) {
      log.warn("demo_bot.engine", { error: String((e as Error)?.message ?? e) });
      return;
    }
    for (const a of acts) {
      if (a.t === "send" && "bytes" in a) {
        if (a.link === LINK) this.relay.onMessage(this.id, tagged(WsTag.Envelope, Uint8Array.from(a.bytes)));
      } else this.opts.onAction(a);
    }
  }

  private onRelay(b: Uint8Array) {
    const now = this.opts.now();
    const body = b.subarray(1);
    if (b[0] === WsTag.Envelope) {
      if (body.length < 16) return;
      const src = hex(body.subarray(8, 16));
      this.seen(src, now);
      this.call((e) => e.on_frame(LINK, src, body, now));
      return;
    }
    let sig: Signal;
    try { sig = fromBinary(SignalSchema, body); } catch { return; }
    const k = sig.kind;
    if (k.case === "roomJoined") {
      if (!this.linkUp) {
        this.linkUp = true;
        this.call((e) => e.on_link_up(LINK, CLASS_INTERNET, now));
        this.call((e) => e.on_link_stats(LINK, 200_000, 120, 0, now));
      }
      for (const p of k.value.peers) this.seen(hex(p.nodeId), now);
    } else if (k.case === "peerEvent" && k.value.node) {
      const id = hex(k.value.node.nodeId);
      if (k.value.joined) this.seen(id, now);
      else if (this.peers.delete(id)) this.call((e) => e.on_peer_lost(LINK, id, now));
    } else if (k.case === "error") {
      log.debug("demo_bot.relay_error", { code: k.value.code, message: k.value.message });
    }
  }

  private seen(id: string, now: number) {
    if (id === this.nodeHex || this.peers.has(id)) return;
    this.peers.add(id);
    this.call((e) => e.on_peer_seen(LINK, id, now));
  }
}

// ---- the bot ----------------------------------------------------------------

export interface DemoBotOptions {
  /** 64 hex chars: the bot's identity seed (secret). */
  seedHex?: string | undefined;
  /** Hex of the engine's persisted "groups" blob (secret: holds group keys). */
  groupsHex?: string | undefined;
  now?: () => number;
  /** Start the 20 ms engine clock (tests drive `tick()` by hand). Default true. */
  autoTick?: boolean;
  wasm?: WasmModule;
}

export class DemoBot {
  readonly peer: InProcessPeer;
  readonly groups: string[];
  private readonly parrot = new Parrot();
  private parrotGroup = "";
  private readonly limiter = new RateLimiter();
  /** group → our newest message id (target of /delete and /edit). */
  private readonly lastOwn = new Map<string, string>();
  private timers: { at: number; fn: () => void }[] = [];
  private interval: NodeJS.Timeout | null = null;

  constructor(relay: Relay, wasm: WasmModule, eng: WasmEngineLike, private readonly now: () => number) {
    this.groups = (JSON.parse(eng.groups_json()) as { id: string }[]).map((g) => g.id);
    this.peer = new InProcessPeer(relay, eng, { name: BOT_NAME, hue: BOT_HUE, now, onAction: (a) => this.onAction(a) });
    if (this.groups[0]) eng.set_active_group(unhex(this.groups[0]));
    this.peer.join(this.groups.map((g) => wasm.group_hash(unhex(g))));
  }

  get nodeHex() { return this.peer.nodeHex; }

  start() {
    this.interval ??= setInterval(() => this.tick(), TICK_MS);
    this.interval.unref();
  }

  stop() {
    if (this.interval) clearInterval(this.interval);
    this.interval = null;
    this.peer.disconnect();
  }

  tick() {
    const now = this.now();
    this.peer.run((e) => e.tick(now));
    const due = this.timers.filter((t) => t.at <= now);
    this.timers = this.timers.filter((t) => t.at > now);
    for (const t of due) t.fn();
    this.stepParrot(now);
  }

  private stepParrot(now: number) {
    const s = this.parrot.step(now, this.peer.eng.is_talking());
    switch (s.t) {
      case "down":
        if (!this.limiter.take(s.talker, now)) { this.parrot.cancel(); return; }
        if (this.parrotGroup) this.peer.eng.set_active_group(unhex(this.parrotGroup));
        this.peer.run((e) => e.ptt_down(0, now));
        return;
      case "packet": this.peer.run((e) => e.on_opus_in(s.packet, now)); return;
      case "up": this.peer.run((e) => e.ptt_up(now)); return;
      default: return;
    }
  }

  private onAction(a: Action) {
    const now = this.now();
    if (a.t === "playPacket" && "talker" in a) {
      if (a.talker !== this.nodeHex) this.parrot.collect(a.talker, Uint8Array.from(a.packet), now);
      return;
    }
    if (a.t !== "ui" || !("event" in a)) return; // persist: in-memory only; capture/wakeAt: n/a
    const ev = a.event;
    if (ev.type === "floorTaken" && "holder" in ev) {
      if (ev.holder !== this.nodeHex) { this.parrot.floorTaken(); this.parrotGroup = ev.group; }
    } else if (ev.type === "floorIdle") {
      this.parrot.floorIdle(now);
    } else if (ev.type === "message" && "body" in ev) {
      if (ev.from === this.nodeHex) this.lastOwn.set(ev.group, ev.msg_uuid);
      else if (ev.body.kind === "text" && typeof ev.body.text === "string") this.onText(ev.group, ev.from, ev.msg_uuid, ev.body.text, now);
    }
  }

  private onText(group: string, from: string, msgId: string, text: string, now: number) {
    if (!this.limiter.take(from, now)) return;
    const g = unhex(group);
    const none = new Uint8Array();
    const cmd = text.trim().toLowerCase();
    if (cmd === "/delete") {
      const last = this.lastOwn.get(group);
      if (last) this.peer.run((e) => e.delete_message(g, unhex(last), now));
      this.lastOwn.delete(group);
      return;
    }
    if (cmd === "/edit") {
      this.peer.run((e) => e.send_text_v2(g, "typing…", none, none, false, now));
      const target = this.lastOwn.get(group);
      if (target) this.timers.push({ at: now + EDIT_DELAY_MS, fn: () => this.peer.run((e) => e.edit_message(g, unhex(target), "Edited by the demo bot ✓", this.now())) });
      return;
    }
    const target = unhex(msgId);
    this.peer.run((e) => e.react(g, target, "👍", now));
    const quote = Array.from(text).slice(0, QUOTE_CHARS).join("");
    this.peer.run((e) => e.send_text_v2(g, `You said: ${quote}`, target, none, false, now));
  }
}

/** Start the demo bot when DEMO_BOT_SEED + DEMO_BOT_GROUPS are configured; otherwise null. */
export function startDemoBot(relay: Relay, opts: DemoBotOptions): DemoBot | null {
  const { seedHex, groupsHex } = opts;
  if (!seedHex || !groupsHex) return null;
  if (!/^[0-9a-f]{64}$/i.test(seedHex) || !/^([0-9a-f]{2})+$/i.test(groupsHex)) {
    log.warn("demo_bot.bad_config", { reason: "seed must be 64 hex chars and groups even-length hex" });
    return null;
  }
  const now = opts.now ?? Date.now;
  const wasm = opts.wasm ?? loadWasm();
  const rng = BigInt(Math.floor(Math.random() * 2 ** 52));
  const eng = new wasm.WasmEngine(unhex(seedHex), BOT_NAME, BOT_HUE, rng);
  if (!eng.restore_groups(unhex(groupsHex), now())) {
    log.warn("demo_bot.bad_config", { reason: "groups blob rejected by the engine" });
    return null;
  }
  const bot = new DemoBot(relay, wasm, eng, now);
  if (opts.autoTick !== false) bot.start();
  log.info("demo_bot.started", { groups: bot.groups.length, node: bot.nodeHex.slice(0, 6) });
  return bot;
}
