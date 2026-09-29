// Browser host for the wasm engine: single instance, actions dispatched to the
// relay transport, WebCodecs audio and the shared reducer.
import { RelayTransport } from "./relay";
import { Capture, Playout, webCodecsSupported } from "./audio";
import { DEFAULT_RELAY, defaultSettings, useStore, type Settings } from "../store";
import { applyUi, setGroups, type CueKind } from "../reducer";
import { LinkClassCode, LinkIds, type Action, type GroupJson } from "../types";
import type { Capabilities, TitiHost } from "../platform";

type Wasm = typeof import("@titi/core-wasm");

const hexToBytes = (h: string) => Uint8Array.from(h.match(/.{2}/g) ?? [], (b) => parseInt(b, 16));
const bytesToHex = (b: Uint8Array) => Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
const now = () => Date.now();

const LS = { seed: "titi.seed", groups: "titi.groups", settings: "titi.settings" } as const;

export class WebHost implements TitiHost {
  readonly kind: "web" | "tv";
  readonly caps: Capabilities;
  private wasm!: Wasm;
  private eng!: InstanceType<Wasm["WasmEngine"]>;
  private relay!: RelayTransport;
  private capture!: Capture;
  readonly playout = new Playout();
  private tick: number | null = null;
  private started = false;
  private pendingCode: { code: string; until: number } | null = null;
  readonly dbg = { rx: 0, tx: 0, play: 0, ui: [] as string[] };
  private hooks = {
    cue: (k: CueKind) => this.playout.cue(k),
    refreshGroups: () => this.refreshGroups(),
    syncRooms: () => this.syncRoom(),
  };

  /** Present only when the platform can end the app (Tizen widget exit). */
  readonly quit?: () => void;

  constructor(opts: { kind?: "web" | "tv"; tenFoot?: boolean; exit?: () => void } = {}) {
    this.kind = opts.kind ?? "web";
    const exit = opts.exit;
    if (exit) this.quit = () => { this.shutdown(); exit(); };
    const hasWindow = typeof window !== "undefined";
    this.caps = {
      lan: false,
      ble: false,
      globalPtt: false,
      qrScan: hasWindow && "BarcodeDetector" in window,
      mic: hasWindow && !!navigator.mediaDevices?.getUserMedia,
      tenFoot: opts.tenFoot ?? false,
    };
  }

  async init() {
    if (this.wasm) return;
    this.wasm = await import("@titi/core-wasm");
    const s = loadSettings();
    const seedHex = localStorage.getItem(LS.seed);
    const seed = seedHex ? hexToBytes(seedHex) : new Uint8Array();
    const rng = BigInt(Math.floor(Math.random() * 2 ** 52));
    this.eng = new this.wasm.WasmEngine(seed, s.name || "Titi", s.hue, rng);
    if (!seedHex) localStorage.setItem(LS.seed, bytesToHex(this.eng.identity_seed()));
    const gb = localStorage.getItem(LS.groups);
    if (gb) this.eng.restore_groups(hexToBytes(gb), now());
    this.capture = new Capture((pkt) => this.apply(this.eng.on_opus_in(pkt, now())));
    this.capture.onLevel = (db) => useStore.getState().set({ levelDbfs: db });
    this.playout.volume = s.volume;
    this.relay = new RelayTransport(
      () => useStore.getState().settings.relayUrl || DEFAULT_RELAY,
      () => this.eng.node_id(),
      () => useStore.getState().settings.name,
      () => useStore.getState().settings.hue,
      {
        linkUp: () => { this.apply(this.eng.on_link_up(LinkIds.INTERNET, LinkClassCode.internet, now())); this.apply(this.eng.on_link_stats(LinkIds.INTERNET, 200_000, 120, 0, now())); },
        linkDown: () => this.apply(this.eng.on_link_down(LinkIds.INTERNET, now())),
        peerSeen: (t) => this.apply(this.eng.on_peer_seen(LinkIds.INTERNET, t, now())),
        peerLost: (t) => this.apply(this.eng.on_peer_lost(LinkIds.INTERNET, t, now())),
        frame: (t, b) => { this.dbg.rx++; this.apply(this.eng.on_frame(LinkIds.INTERNET, t, b, now())); },
      },
    );
    (window as unknown as { __titi: unknown }).__titi = this;
    useStore.getState().set({ ready: true, nodeId: bytesToHex(this.eng.node_id()), settings: s, voiceSupported: webCodecsSupported(), platform: this.kind });
    this.refreshGroups();
  }

  async start() {
    if (this.started) return;
    this.started = true;
    await this.playout.ensure();
    this.relay.start();
    let n = 0;
    this.tick = window.setInterval(() => {
      this.apply(this.eng.tick(now()));
      this.playout.dropIdle();
      if (++n % 500 === 0) this.syncRoom(); // every 10 s: rendezvous slots rotate
    }, 20);
    useStore.getState().set({ running: true });
    this.syncRoom();
  }

  stopTicking() { if (this.tick) { clearInterval(this.tick); this.tick = null; } }

  /** Release the floor, close mic + relay and stop the engine clock (before the app exits). */
  shutdown() {
    if (!this.started) return;
    try { this.apply(this.eng.ptt_up(now())); } catch { /* engine already gone */ }
    void this.capture.stop();
    this.relay.stop();
    this.stopTicking();
    this.started = false;
    useStore.getState().set({ running: false });
  }

  saveSettings(p: Partial<Settings>) {
    const s = { ...useStore.getState().settings, ...p };
    localStorage.setItem(LS.settings, JSON.stringify(s));
    useStore.getState().set({ settings: s });
    this.eng.set_display_name(s.name, s.hue);
    this.playout.volume = s.volume;
  }

  // ---- intents -----------------------------------------------------------
  pttDown() { this.apply(this.eng.ptt_down(0, now())); }
  pttUp() { this.apply(this.eng.ptt_up(now())); }
  setMuted(m: boolean) { this.capture.muted = m; useStore.getState().set({ muted: m }); }
  createGroup(name: string) { this.guard(() => this.apply(this.eng.create_group(name, now()))); this.refreshGroups(); this.syncRoom(); }
  leaveGroup(id: string) { this.guard(() => this.apply(this.eng.leave_group(hexToBytes(id), now()))); this.refreshGroups(); this.syncRoom(); }
  setActiveGroup(id: string) { try { this.eng.set_active_group(hexToBytes(id)); } catch { /* unknown */ } this.refreshGroups(); this.syncRoom(); useStore.getState().updateGroup(id, (g) => ({ ...g, unread: 0 })); }
  setFullDuplex(id: string, on: boolean) { this.guard(() => this.apply(this.eng.set_full_duplex(hexToBytes(id), on, now()))); this.refreshGroups(); }
  invitePeer(g: string, node: string) { this.guard(() => this.apply(this.eng.invite_peer(hexToBytes(g), hexToBytes(node), now()))); }
  acceptInvite(g: string, h: string) { dropInvite(g); this.guard(() => this.apply(this.eng.accept_invite(hexToBytes(g), hexToBytes(h), now()))); }
  declineInvite(g: string, h: string) { dropInvite(g); try { this.apply(this.eng.decline_invite(hexToBytes(g), hexToBytes(h), now())); } catch { /* ignore */ } }
  joinByCode(code: string) {
    // Sit in the code's rendezvous rooms for 2 minutes so the host (online in
    // the same rooms) becomes a neighbour; retry the join once peers appear.
    this.pendingCode = { code, until: now() + 120_000 };
    const before = new Set(useStore.getState().groups.map((g) => g.id));
    this.syncRoom();
    this.apply(this.eng.join_by_code(code, now()));
    const retry = window.setInterval(() => {
      const done = useStore.getState().groups.some((g) => !before.has(g.id));
      if (!this.pendingCode || now() > this.pendingCode.until || done) {
        clearInterval(retry);
        if (this.pendingCode && (done || now() > this.pendingCode.until)) { this.pendingCode = null; this.syncRoom(); }
        return;
      }
      this.apply(this.eng.join_by_code(code, now()));
    }, 3000);
  }
  joinByLink(url: string) { this.apply(this.eng.join_by_link(url, now())); }
  sendText(g: string, text: string) { this.guard(() => this.apply(this.eng.send_text(hexToBytes(g), text, now()))); }
  sendSos(g: string, cancelled: boolean) {
    const fire = (lat: number, lon: number) => this.guard(() => this.apply(this.eng.send_sos(hexToBytes(g), lat, lon, "", cancelled, now())));
    if (cancelled || !navigator.geolocation) { fire(0, 0); return; }
    navigator.geolocation.getCurrentPosition((p) => fire(p.coords.latitude, p.coords.longitude), () => fire(0, 0), { timeout: 5000, maximumAge: 60_000 });
  }
  async currentCode(g: string): Promise<[string, number] | null> { const c = this.eng.current_code(hexToBytes(g), now()); if (!c) return null; const [code, secs] = c.split("|"); return [code!, Number(secs ?? 0)]; }
  async deepLink(g: string): Promise<string | null> { return this.eng.deep_link(hexToBytes(g), now(), 10 * 60_000) ?? null; }
  async parseCode(t: string) { return this.wasm ? this.wasm.parse_invite_code(t) : false; }

  // ---- dispatch ----------------------------------------------------------
  private apply(json: string) {
    let acts: Action[];
    try { acts = JSON.parse(json); } catch { return; }
    for (const a of acts) {
      switch (a.t) {
        case "send": if (a.link === LinkIds.INTERNET) { this.dbg.tx++; this.relay.send(Uint8Array.from(a.bytes)); } break;
        case "playPacket": this.dbg.play++; this.playout.play(a.talker, Uint8Array.from(a.packet), a.frames); break;
        case "capture": if (a.active) this.capture.start(a.profile).catch((e: unknown) => this.onMicError(e)); else void this.capture.stop(); break;
        case "persist": if (a.key === "groups") localStorage.setItem(LS.groups, bytesToHex(Uint8Array.from(a.value))); break;
        case "ui": if (a.event.type !== "level" && a.event.type !== "peerLink") this.dbg.ui.push(a.event.type); applyUi(a.event, this.hooks); break;
        case "wakeAt": break;
      }
    }
  }

  private onMicError(e: unknown) {
    this.apply(this.eng.ptt_up(now()));
    useStore.getState().set({ toast: `Microphone unavailable: ${String((e as Error)?.message ?? e)}` });
  }

  private refreshGroups() { setGroups(JSON.parse(this.eng.groups_json()) as GroupJson[]); }

  private syncRoom() {
    const st = useStore.getState();
    const rooms: { hash: Uint8Array; rendezvous: boolean }[] = [];
    for (const g of st.groups) {
      const gid = hexToBytes(g.id);
      rooms.push({ hash: this.wasm.group_hash(gid), rendezvous: false });
      // every member hosts joins-by-code (any member can be asked for the code)
      for (const h of chunk4(this.eng.rendezvous_for_group(gid, now()))) rooms.push({ hash: h, rendezvous: true });
    }
    if (this.pendingCode) for (const h of chunk4(this.wasm.rendezvous_for_code(this.pendingCode.code, now()))) rooms.push({ hash: h, rendezvous: true });
    this.relay.setRooms(rooms);
  }

  private guard(f: () => void) { try { f(); } catch (e) { useStore.getState().set({ toast: String((e as Error)?.message ?? e) }); } }
}

function dropInvite(g: string) { useStore.getState().set((s) => ({ invites: s.invites.filter((i) => i.group !== g) })); }
function chunk4(b: Uint8Array): Uint8Array[] { const out: Uint8Array[] = []; for (let i = 0; i + 4 <= b.length; i += 4) out.push(b.slice(i, i + 4)); return out; }

function loadSettings(): Settings {
  try { return { ...defaultSettings, ...JSON.parse(localStorage.getItem(LS.settings) ?? "{}") }; } catch { return { ...defaultSettings }; }
}
