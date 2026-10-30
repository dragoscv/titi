// Desktop host: every intent is a Tauri command into the native Rust engine;
// UI events come back as the same JSON the wasm build emits, so the shared
// reducer renders them unchanged.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { applyUi, setGroups, useStore, defaultSettings, type Capabilities, type ChatItem, type EditVersion, type GroupJson, type SendTextOpts, type Settings, type TitiHost, type UiEvent, type AudioDevice, type CueKind } from "@titi/app-ui";

interface InitInfo { nodeId: string; settings: Settings; groups: string; platform: string }
/** Arguments of the `titi_chat` command (snake_case, hex ids; omitted = unused). */
interface ChatArgs { group?: string; target?: string; text?: string; to_group?: string; mentions?: string[]; mention_all?: boolean; before_ms?: number; limit?: number }

export class DesktopHost implements TitiHost {
  readonly kind = "desktop" as const;
  readonly caps: Capabilities = { lan: true, ble: true, globalPtt: true, qrScan: false, mic: true, tenFoot: false };
  private inited = false;
  private hooks = {
    cue: (k: CueKind) => void invoke("titi_cue", { kind: k }),
    refreshGroups: () => {},
    syncRooms: () => {},
    // toasts are raised natively (actionable, work with the window hidden) — see winshell.rs
  };

  async init() {
    if (this.inited) return;
    this.inited = true;
    await listen<string>("titi://ui", (e) => applyUi(JSON.parse(e.payload) as UiEvent, this.hooks));
    await listen<string>("titi://groups", (e) => setGroups(JSON.parse(e.payload) as GroupJson[]));
    await listen<number>("titi://level", (e) => useStore.getState().set({ levelDbfs: e.payload }));
    await listen<boolean>("titi://ptt-key", (e) => useStore.getState().set({ pttKeyDown: e.payload }));
    await listen<string>("titi://deeplink", (e) => useStore.getState().set({ pendingLink: e.payload }));
    await listen<string>("titi://route", (e) => useStore.getState().set({ pendingRoute: e.payload }));
    await listen<boolean>("titi://muted", (e) => useStore.getState().set({ muted: e.payload }));
    await listen("titi://toggle-mute", () => this.setMuted(!useStore.getState().muted));
    const info = await invoke<InitInfo>("titi_init");
    useStore.getState().set({ ready: true, running: true, nodeId: info.nodeId, settings: { ...defaultSettings, ...info.settings }, platform: "desktop", voiceSupported: true });
    setGroups(JSON.parse(info.groups) as GroupJson[]);
  }

  async start() { /* the native radio is always on */ }

  saveSettings(p: Partial<Settings>) {
    const s = { ...useStore.getState().settings, ...p };
    useStore.getState().set({ settings: s });
    void invoke("titi_save_settings", { settings: s }).catch(this.err);
  }

  pttDown() { void invoke("titi_ptt", { down: true }); }
  pttUp() { void invoke("titi_ptt", { down: false }); }
  setMuted(m: boolean) { useStore.getState().set({ muted: m }); void invoke("titi_mute", { muted: m }); }
  createGroup(name: string) { void invoke("titi_create_group", { name }).catch(this.err); }
  private op(op: string, group: string, arg?: string, flag?: boolean) { void invoke("titi_group_op", { op, group, arg, flag }).catch(this.err); }
  leaveGroup(id: string) { this.op("leave", id); }
  dissolveGroup(id: string) { this.op("dissolve", id); }
  setActiveGroup(id: string) { this.op("active", id); useStore.getState().updateGroup(id, (g) => ({ ...g, unread: 0 })); }
  setFullDuplex(id: string, on: boolean) { this.op("duplex", id, undefined, on); }
  invitePeer(g: string, node: string) { this.op("invite", g, node); }
  acceptInvite(g: string, h: string) { dropInvite(g); this.op("accept", g, h); }
  declineInvite(g: string, h: string) { dropInvite(g); this.op("decline", g, h); }
  joinByCode(code: string) { void invoke("titi_join", { code }).catch(this.err); }
  joinByLink(link: string) { void invoke("titi_join", { link }).catch(this.err); }
  sendText(g: string, text: string) { this.op("text", g, text); }
  // ---- chat v2 (native store: chat.db) -----------------------------------
  private chat<T = null>(op: string, args: ChatArgs) { return invoke<T>("titi_chat", { op, args }); }
  private chatDo(op: string, args: ChatArgs) { void this.chat(op, args).catch(this.err); }
  sendTextV2(g: string, text: string, o: SendTextOpts = {}) { this.chatDo("reply", { group: g, text, target: o.replyTo, mentions: o.mentions, mention_all: o.mentionAll }); }
  react(g: string, target: string, emoji: string) { this.chatDo("react", { group: g, target, text: emoji }); }
  editMessage(g: string, target: string, text: string) { this.chatDo("edit", { group: g, target, text }); }
  deleteMessage(g: string, target: string) { this.chatDo("delete", { group: g, target }); }
  forward(from: string, target: string, to: string) { this.chatDo("forward", { group: from, target, to_group: to }); }
  setDraft(g: string, text: string) { this.chatDo("draft", { group: g, text }); }
  async draft(g: string) { try { return (await this.chat<string | null>("getDraft", { group: g })) ?? null; } catch { return null; } }
  async history(g: string, beforeMs?: number) { return this.list<ChatItem>("history", { group: g, before_ms: beforeMs, limit: 200 }); }
  async search(g: string | null, q: string) { return this.list<ChatItem>("search", { group: g ?? "", text: q, limit: 50 }); }
  async editHistory(target: string) { return this.list<EditVersion>("editHistory", { target }); }
  private async list<T>(op: string, args: ChatArgs): Promise<T[]> {
    try { const v = await this.chat<T[] | null>(op, args); return Array.isArray(v) ? v : []; } catch (e) { this.err(e); return []; }
  }
  sendSos(g: string, cancelled: boolean) { this.op("sos", g, undefined, cancelled); if (!cancelled) void invoke("titi_ptt", { down: true, prio: 2 }); }
  async currentCode(g: string) { return invoke<[string, number] | null>("titi_code", { group: g }); }
  async deepLink(g: string) { return invoke<string | null>("titi_deep_link", { group: g }); }
  async parseCode(text: string) { return invoke<boolean>("titi_parse_code", { text }); }
  async audioDevices() { return invoke<{ inputs: AudioDevice[]; outputs: AudioDevice[] }>("titi_devices"); }
  async capturePttKey() { return invoke<string | null>("titi_learn_ptt"); }
  quit() { void invoke("titi_quit"); }

  private err = (e: unknown) => useStore.getState().set({ toast: String(e) });
}

function dropInvite(g: string) { useStore.getState().set((s) => ({ invites: s.invites.filter((i) => i.group !== g) })); }
