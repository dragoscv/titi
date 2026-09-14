"use client";
// Browser host for the wasm engine: single instance, actions dispatched to the
// relay transport, WebCodecs audio and the zustand store.
import { RelayTransport } from "./relay";
import { Capture, Playout } from "./audio";
import { DEFAULT_RELAY, useStore, type GroupState, type Msg, type Settings } from "./store";
import { LinkClassCode, LinkIds, type Action, type GroupJson, type UiEvent } from "./types";

type Wasm = typeof import("@titi/core-wasm");

const hexToBytes = (h: string) => Uint8Array.from(h.match(/.{2}/g)!.map((b) => parseInt(b, 16)));
const bytesToHex = (b: Uint8Array) => Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
const now = () => Date.now();

const LS = { seed: "titi.seed", groups: "titi.groups", settings: "titi.settings" } as const;

class Host {
  private wasm!: Wasm;
  private eng!: InstanceType<Wasm["WasmEngine"]>;
  private relay!: RelayTransport;
  private capture!: Capture;
  readonly playout = new Playout();
  private tick: number | null = null;
  stopTicking() { if (this.tick) { clearInterval(this.tick); this.tick = null; } }
  private names = new Map<string, string>();
  private started = false;
  private pendingCode: { code: string; until: number } | null = null;
  readonly dbg = { rx: 0, tx: 0, play: 0, ui: [] as string[] };

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
    if (process.env.NODE_ENV !== "production") (window as unknown as { __titi: unknown }).__titi = this;
    useStore.getState().set({ ready: true, nodeId: bytesToHex(this.eng.node_id()), settings: s });
    this.refreshGroups();
  }

  /** Must be called from a user gesture (AudioContext + mic). */
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

  saveSettings(p: Partial<Settings>) {
    const s = { ...useStore.getState().settings, ...p };
    localStorage.setItem(LS.settings, JSON.stringify(s));
    useStore.getState().set({ settings: s });
    this.eng.set_display_name(s.name, s.hue);
  }

  // ---- intents -----------------------------------------------------------
  pttDown() { this.apply(this.eng.ptt_down(0, now())); }
  pttUp() { this.apply(this.eng.ptt_up(now())); }
  setMuted(m: boolean) { this.capture.muted = m; useStore.getState().set({ muted: m }); }
  createGroup(name: string) { try { this.apply(this.eng.create_group(name, now())); } catch (e) { this.err(e); } this.refreshGroups(); this.syncRoom(); }
  leaveGroup(id: string) { try { this.apply(this.eng.leave_group(hexToBytes(id), now())); } catch (e) { this.err(e); } this.refreshGroups(); this.syncRoom(); }
  setActiveGroup(id: string) { try { this.eng.set_active_group(hexToBytes(id)); } catch { /* unknown */ } this.refreshGroups(); this.syncRoom(); useStore.getState().updateGroup(id, (g) => ({ ...g, unread: 0 })); }
  setFullDuplex(id: string, on: boolean) { try { this.apply(this.eng.set_full_duplex(hexToBytes(id), on, now())); } catch (e) { this.err(e); } this.refreshGroups(); }
  invitePeer(g: string, node: string) { try { this.apply(this.eng.invite_peer(hexToBytes(g), hexToBytes(node), now())); } catch (e) { this.err(e); } }
  acceptInvite(g: string, host: string) { useStore.getState().set((s) => ({ invites: s.invites.filter((i) => i.group !== g) })); try { this.apply(this.eng.accept_invite(hexToBytes(g), hexToBytes(host), now())); } catch (e) { this.err(e); } }
  declineInvite(g: string, host: string) { useStore.getState().set((s) => ({ invites: s.invites.filter((i) => i.group !== g) })); try { this.apply(this.eng.decline_invite(hexToBytes(g), hexToBytes(host), now())); } catch { /* ignore */ } }
  joinByCode(code: string) {
    // Sit in the code's rendezvous rooms for 2 minutes so the host (online in
    // the same rooms) becomes a neighbour; retry the join once peers appear.
    this.pendingCode = { code, until: now() + 120_000 };
    this.syncRoom();
    this.apply(this.eng.join_by_code(code, now()));
    const retry = window.setInterval(() => {
      if (!this.pendingCode || now() > this.pendingCode.until || useStore.getState().groups.some((g) => g.isActive && g.id === useStore.getState().activeGroup && g.memberCount > 1)) {
        clearInterval(retry);
        if (this.pendingCode && now() > this.pendingCode.until) { this.pendingCode = null; this.syncRoom(); }
        return;
      }
      this.apply(this.eng.join_by_code(code, now()));
    }, 3000);
  }
  joinByLink(url: string) { this.apply(this.eng.join_by_link(url, now())); }
  sendText(g: string, text: string) { try { this.apply(this.eng.send_text(hexToBytes(g), text, now())); } catch (e) { this.err(e); } }
  currentCode(g: string): [string, number] | null { const c = this.eng.current_code(hexToBytes(g), now()); if (!c) return null; const [code, secs] = c.split("|"); return [code!, Number(secs ?? 0)]; }
  deepLink(g: string): string | null { return this.eng.deep_link(hexToBytes(g), now(), 10 * 60_000) ?? null; }
  parseCode(t: string) { return this.wasm.parse_invite_code(t); }

  // ---- dispatch ----------------------------------------------------------
  private apply(json: string) {
    let acts: Action[];
    try { acts = JSON.parse(json); } catch { return; }
    for (const a of acts) {
      switch (a.t) {
        case "send": if (a.link === LinkIds.INTERNET) { this.dbg.tx++; this.relay.send(Uint8Array.from(a.bytes)); } break;
        case "playPacket": this.dbg.play++; this.playout.play(a.talker, Uint8Array.from(a.packet), a.frames); break;
        case "capture": if (a.active) void this.capture.start(a.profile); else void this.capture.stop(); break;
        case "persist": if (a.key === "groups") localStorage.setItem(LS.groups, bytesToHex(Uint8Array.from(a.value))); break;
        case "ui": if (a.event.type !== "level" && a.event.type !== "peerLink") this.dbg.ui.push(a.event.type); this.onUi(a.event); break;
        case "wakeAt": break;
      }
    }
  }

  private onUi(e: UiEvent) {
    const st = useStore.getState();
    const me = st.nodeId;
    switch (e.type) {
      case "peerDiscovered": this.names.set(e.node, e.name); st.set((s) => ({ peers: { ...s.peers, [e.node]: { ...(s.peers[e.node] ?? { bars: 0, hops: 0 }), node: e.node, name: e.name, hue: e.hue, link: e.link, inGroup: e.in_group } } })); break;
      case "peerLost": st.set((s) => { const p = { ...s.peers }; delete p[e.node]; return { peers: p }; }); break;
      case "peerLink": st.set((s) => s.peers[e.node] ? { peers: { ...s.peers, [e.node]: { ...s.peers[e.node]!, link: e.link, bars: e.bars, hops: e.hops } } } : {}); break;
      case "floorGranted": this.updActive((g) => ({ ...g, floor: "talking", talkerName: null, talkerNode: me })); this.playout.cue("granted"); break;
      case "floorDenied": this.updActive((g) => ({ ...g, floor: "busy" })); this.playout.cue("denied"); break;
      case "floorTaken": this.names.set(e.holder, e.name); st.updateGroup(e.group, (g) => ({ ...g, floor: e.holder === me ? "talking" : "busy", talkerName: e.name, talkerNode: e.holder })); if (e.holder !== me) this.playout.cue("incoming"); break;
      case "floorIdle": { const wasMine = st.groups.find((g) => g.id === e.group)?.talkerNode === me; st.updateGroup(e.group, (g) => ({ ...g, floor: "idle", talkerName: null, talkerNode: null })); if (wasMine) this.playout.cue("released"); break; }
      case "talkWarning": st.set({ toast: "10 seconds left" }); break;
      case "talkTimeout": st.set({ toast: "Talk time limit reached" }); break;
      case "inviteOffered": st.set((s) => ({ invites: [...s.invites.filter((i) => i.group !== e.group), { group: e.group, name: e.name, host: e.host, hostName: e.host_name, members: e.members }] })); this.playout.cue("incoming"); break;
      case "joined": this.refreshGroups(); this.syncRoom(); st.set({ toast: `Joined ${e.name}` }); break;
      case "joinFailed": st.set({ toast: `Could not join: ${e.reason}` }); break;
      case "memberJoined": this.names.set(e.node, e.name); this.refreshGroups(); break;
      case "memberLeft": this.refreshGroups(); break;
      case "message": { const m: Msg = { id: e.msg_uuid, group: e.group, from: e.from, fromName: this.names.get(e.from) ?? e.from.slice(0, 6), sentMs: e.sent_ms, body: e.body, mine: e.from === me, acked: false }; st.set((s) => ({ messages: [...s.messages, m].slice(-500) })); if (!m.mine && st.activeGroup !== e.group) st.updateGroup(e.group, (g) => ({ ...g, unread: g.unread + 1 })); break; }
      case "messageAcked": st.set((s) => ({ messages: s.messages.map((m) => (m.id === e.msg_uuid ? { ...m, acked: true } : m)) })); break;
      case "handover": st.updateGroup(e.group, (g) => ({ ...g, handover: e.state, link: e.link, profile: e.profile })); break;
      case "suspended": st.updateGroup(e.group, (g) => ({ ...g, suspended: true })); break;
      case "resumed": st.updateGroup(e.group, (g) => ({ ...g, suspended: false })); break;
      case "modeChanged": st.updateGroup(e.group, (g) => ({ ...g, fullDuplex: e.full_duplex })); break;
      case "level": if (e.talker !== me) st.set({ levelDbfs: e.dbfs }); break;
      case "error": st.set({ toast: e.message }); break;
    }
  }

  private updActive(f: (g: GroupState) => GroupState) { const a = useStore.getState().activeGroup; if (a) useStore.getState().updateGroup(a, f); }

  private refreshGroups() {
    const gs: GroupJson[] = JSON.parse(this.eng.groups_json());
    const prev = new Map(useStore.getState().groups.map((g) => [g.id, g]));
    const groups: GroupState[] = gs.map((g) => {
      g.members.forEach((m) => this.names.set(m.node, m.name));
      const p = prev.get(g.id);
      return { ...(p ?? { floor: "idle", talkerName: null, talkerNode: null, handover: "stable", link: null, profile: "std", suspended: false, unread: 0 }), ...g } as GroupState;
    });
    useStore.getState().set({ groups, activeGroup: gs.find((g) => g.isActive)?.id ?? null });
  }

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

  private err(e: unknown) { useStore.getState().set({ toast: String((e as Error)?.message ?? e) }); }
}

function chunk4(b: Uint8Array): Uint8Array[] { const out: Uint8Array[] = []; for (let i = 0; i + 4 <= b.length; i += 4) out.push(b.slice(i, i + 4)); return out; }

function loadSettings(): Settings {
  try { return { name: "", hue: 40, onboarded: false, relayUrl: DEFAULT_RELAY, ...JSON.parse(localStorage.getItem(LS.settings) ?? "{}") }; } catch { return { name: "", hue: 40, onboarded: false, relayUrl: DEFAULT_RELAY }; }
}

export const host = new Host();
