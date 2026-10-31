import { beforeEach, describe, expect, it } from "vitest";
import { loadConfig } from "../src/config.ts";
import {
  BOT_NAME,
  hex,
  InProcessPeer,
  loadWasm,
  Parrot,
  PARROT_IDLE_MS,
  PARROT_MAX_PACKETS,
  RateLimiter,
  startDemoBot,
  unhex,
  type WasmEngineLike,
} from "../src/demo-bot.ts";
import { setLogLevel } from "../src/log.ts";
import { Relay } from "../src/relay.ts";

beforeEach(() => setLogLevel("error"));

const wasm = loadWasm();
type FullEngine = WasmEngineLike & {
  create_group(name: string, now: number): string;
  deep_link(group: Uint8Array, now: number, validMs: number): string | undefined;
  join_by_link(url: string, now: number): string;
};
const engine = (name: string, hue: number, rng: bigint) => new wasm.WasmEngine(new Uint8Array(), name, hue, rng) as FullEngine;
const T0 = 1_800_000_000_000;

describe("RateLimiter", () => {
  it("allows 10 actions per sender per minute and refills over time", () => {
    const rl = new RateLimiter();
    const ok = Array.from({ length: 12 }, () => rl.take("a", 0));
    expect(ok.filter(Boolean)).toHaveLength(10);
    expect(rl.take("b", 0)).toBe(true); // other senders unaffected
    expect(rl.take("a", 6_000)).toBe(true); // 6 s = one token back
    expect(rl.take("a", 6_000)).toBe(false);
  });

  it("caps all senders together at 60 per minute", () => {
    const rl = new RateLimiter();
    let ok = 0;
    for (let i = 0; i < 100; i++) if (rl.take(`s${i}`, 0)) ok++;
    expect(ok).toBe(60);
  });
});

const pkt = (n: number) => new Uint8Array([n]);

describe("Parrot", () => {
  it("replays a turn 700 ms after the floor goes idle, one packet per step, then releases", () => {
    const p = new Parrot();
    p.floorTaken();
    for (let i = 0; i < 3; i++) p.collect("aa", pkt(i), 100 + i * 20);
    p.collect("bb", pkt(9), 150); // second talker ignored
    p.floorIdle(200);
    expect(p.step(200 + PARROT_IDLE_MS - 1, false)).toEqual({ t: "none" });
    expect(p.step(200 + PARROT_IDLE_MS, false)).toEqual({ t: "down", talker: "aa" });
    expect(p.step(1_000, false)).toEqual({ t: "wait" }); // arbitration window
    const out = [p.step(1_020, true), p.step(1_040, true), p.step(1_060, true)];
    expect(out).toEqual([0, 1, 2].map((n) => ({ t: "packet", packet: pkt(n) })));
    expect(p.step(1_080, true)).toEqual({ t: "up" });
    expect(p.step(5_000, false)).toEqual({ t: "none" });
  });

  it("caps a turn at 30 s, ignores input while replaying and gives up without a grant", () => {
    const p = new Parrot();
    for (let i = 0; i < PARROT_MAX_PACKETS + 50; i++) p.collect("aa", pkt(1), i * 20);
    expect(p.recorded).toBe(PARROT_MAX_PACKETS);
    const last = (PARROT_MAX_PACKETS + 49) * 20;
    expect(p.step(last + 2_000, false).t).toBe("down"); // silence fallback without floorIdle
    p.collect("aa", pkt(2), last + 2_010);
    expect(p.recorded).toBe(0);
    expect(p.step(last + 3_000, false)).toEqual({ t: "wait" });
    expect(p.step(last + 4_100, false)).toEqual({ t: "up" }); // floor never granted
    expect(p.busy).toBe(false);
  });

  it("drops a pending turn on cancel and keeps waiting when someone takes the floor again", () => {
    const p = new Parrot();
    p.collect("aa", pkt(1), 100);
    p.floorIdle(120);
    p.floorTaken(); // another talker before the idle delay: no replay yet
    expect(p.step(120 + PARROT_IDLE_MS, false)).toEqual({ t: "none" });
    p.floorIdle(200);
    p.cancel();
    expect(p.recorded).toBe(0);
    expect(p.step(10_000, false)).toEqual({ t: "none" });
    p.floorIdle(10_000); // empty buffer: nothing armed
    expect(p.step(20_000, false)).toEqual({ t: "none" });
  });
});

describe("startDemoBot", () => {
  it("is a no-op without both secrets and rejects malformed ones", () => {
    const relay = new Relay(loadConfig({}));
    expect(startDemoBot(relay, {})).toBeNull();
    expect(startDemoBot(relay, { seedHex: "ab".repeat(32) })).toBeNull();
    expect(startDemoBot(relay, { seedHex: "xyz", groupsHex: "00" })).toBeNull();
    expect(startDemoBot(relay, { seedHex: "ab".repeat(32), groupsHex: "ffff", wasm })).toBeNull();
    expect(relay.stats().connections).toBe(0);
  });
});

// ---- end to end: a guest engine joins the bot's group by link ----------------

type Ui = { type: string; [k: string]: unknown };

function setup() {
  let t = T0;
  const now = () => t;
  const relay = new Relay(loadConfig({}), now);
  // the owner's one-off: fresh identity + group (scripts/demo-group-init.mjs)
  const creator = engine(BOT_NAME, 200, 7n);
  const acts = JSON.parse(creator.create_group("Titi Demo", t)) as { t: string; key?: string; value?: number[]; event?: Ui }[];
  const blob = acts.find((a) => a.t === "persist" && a.key === "groups")!.value!;
  const gid = acts.find((a) => a.t === "ui")!.event!.group as string;
  const link = creator.deep_link(unhex(gid), t, 2 * 365 * 86_400_000)!;
  const bot = startDemoBot(relay, { seedHex: hex(creator.identity_seed()), groupsHex: hex(Uint8Array.from(blob)), now, autoTick: false, wasm })!;

  const ui: Ui[] = [];
  const played: string[] = [];
  const guestEng = engine("Reviewer", 30, 9n);
  const guest = new InProcessPeer(relay, guestEng, {
    name: "Reviewer",
    hue: 30,
    now,
    onAction: (a) => {
      if (a.t === "ui" && "event" in a) ui.push(a.event as Ui);
      if (a.t === "playPacket" && "talker" in a) played.push(a.talker);
    },
  });
  guest.join([wasm.group_hash(unhex(gid))]);
  const step = (ms: number) => {
    for (let i = 0; i < ms / 20; i++) {
      t += 20;
      bot.tick();
      guest.run((e) => e.tick(t));
    }
  };
  // like WebHost.joinByLink: retry every 3 s until a member answers
  for (let i = 0; i < 5 && !ui.some((e) => e.type === "joined"); i++) {
    guest.run(() => guestEng.join_by_link(link, t));
    step(3_000);
  }
  return { relay, bot, guest, gid, ui, played, step, now };
}

const texts = (ui: Ui[], from: string) =>
  ui.filter((e) => e.type === "message" && e.from === from).map((e) => (e.body as { text?: string }).text);

describe("DemoBot end to end (in-process relay)", () => {
  it("lets a guest join by link, reacts 👍 and quotes a text message", () => {
    const { bot, guest, gid, ui, step, now } = setup();
    expect(ui.some((e) => e.type === "joined" && e.group === gid)).toBe(true);
    const g = unhex(gid);
    guest.run((e) => e.send_text_v2(g, "hello demo", new Uint8Array(), new Uint8Array(), false, now()));
    step(200);
    expect(texts(ui, bot.nodeHex)).toEqual(["You said: hello demo"]);
    const mine = ui.find((e) => e.type === "message" && e.from === guest.nodeHex)!;
    const updated = ui.filter((e) => e.type === "chatUpdated").map((e) => e.item as { msg_uuid: string; reactions: { emoji: string }[]; reply: { msg_uuid: string } | null });
    expect(updated.some((i) => i.msg_uuid === mine.msg_uuid && i.reactions.some((r) => r.emoji === "👍"))).toBe(true);
    expect(updated.some((i) => i.reply?.msg_uuid === mine.msg_uuid)).toBe(true);
  });

  it("handles /edit and /delete and stops answering a sender after 10 actions a minute", () => {
    const { bot, guest, gid, ui, step, now } = setup();
    const g = unhex(gid);
    const say = (s: string) => { guest.run((e) => e.send_text_v2(g, s, new Uint8Array(), new Uint8Array(), false, now())); step(100); };
    say("/edit");
    expect(texts(ui, bot.nodeHex)).toEqual(["typing…"]);
    step(1_600);
    const items = ui.filter((e) => e.type === "chatUpdated").map((e) => e.item as { from: string; body: { text?: string } | null; edited: number; deleted: boolean });
    expect(items.some((i) => i.from === bot.nodeHex && i.edited > 0 && i.body?.text === "Edited by the demo bot ✓")).toBe(true);
    say("/delete");
    const after = ui.filter((e) => e.type === "chatUpdated").map((e) => e.item as { from: string; deleted: boolean });
    expect(after.some((i) => i.from === bot.nodeHex && i.deleted)).toBe(true);
    for (let i = 0; i < 10; i++) say(`m${i}`);
    // 2 actions used above; 8 more replies, then silence
    expect(texts(ui, bot.nodeHex).filter((s) => s?.startsWith("You said:"))).toHaveLength(8);
  });

  it("parrots a voice turn back to the talker", () => {
    const { bot, guest, played, step, now } = setup();
    guest.run((e) => e.ptt_down(0, now()));
    step(300); // arbitration
    expect(guest.eng.is_talking()).toBe(true);
    for (let i = 0; i < 25; i++) { guest.run((e) => e.on_opus_in(new Uint8Array([0xf8, i, 1, 2]), now())); step(20); }
    guest.run((e) => e.ptt_up(now()));
    step(3_000);
    expect(played.filter((p) => p === bot.nodeHex).length).toBeGreaterThanOrEqual(20);
  });

  it("starts its own ticker, forgets a peer that leaves, survives relay errors and stops cleanly", () => {
    const { relay, bot, guest } = setup();
    bot.start();
    bot.start(); // idempotent
    expect(relay.stats().connections).toBe(2);
    guest.disconnect(); // peerEvent joined=false reaches the bot
    expect(relay.stats().connections).toBe(1);
    // a room-full / bad-frame error from the relay is logged, not thrown
    const big = new Uint8Array(5000);
    big[0] = 0x7f;
    expect(() => relay.onMessage(1, big)).not.toThrow();
    bot.stop();
    expect(relay.stats().connections).toBe(0);
  });

  it("ignores an engine call that returns invalid JSON", () => {
    const relay = new Relay(loadConfig({}));
    const real = engine("x", 1, 1n);
    const bad = Object.create(real) as typeof real;
    bad.tick = () => "not json";
    const peer = new InProcessPeer(relay, bad, { name: "x", hue: 1, now: () => T0, onAction: () => {} });
    expect(() => peer.run((e) => e.tick(T0))).not.toThrow();
  });
});
