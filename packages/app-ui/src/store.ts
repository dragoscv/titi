import { create } from "zustand";
import type { GroupJson, LinkClass, MsgBody, Profile } from "./types";
import type { PlatformKind } from "./platform";

export type FloorState = "idle" | "pending" | "talking" | "busy" | "queued";

export interface Peer { node: string; name: string; hue: number; link: LinkClass; inGroup: boolean; bars: number; hops: number }
export interface Invite { group: string; name: string; host: string; hostName: string; members: number }
export interface Msg { id: string; group: string; from: string; fromName: string; sentMs: number; body: MsgBody; mine: boolean; acked: boolean }
export interface GroupState extends GroupJson {
  floor: FloorState;
  talkerName: string | null;
  talkerNode: string | null;
  handover: string;
  link: LinkClass | null;
  profile: Profile;
  suspended: boolean;
  unread: number;
}

export interface Settings {
  name: string;
  hue: number;
  onboarded: boolean;
  relayUrl: string;
  /** Playback volume 0..1 */
  volume: number;
  /** Desktop: global hold-to-talk key (e.g. "F13", "Mouse4", "CapsLock"), "" = off */
  pttKey: string;
  /** Desktop: floating "who is talking" pill */
  overlay: boolean;
  /** Desktop: launch at login, minimised to tray */
  autostart: boolean;
  /** Desktop: the window close button keeps Titi in the tray (else it quits) */
  closeToTray: boolean;
  /** Desktop/TV: selected audio devices ("" = system default) */
  inputDevice: string;
  outputDevice: string;
  /** LAN transport on/off (native hosts) */
  lan: boolean;
  /** Internet relay on/off */
  relay: boolean;
  /** Bluetooth LE on/off (native hosts with caps.ble) */
  ble: boolean;
}
export const DEFAULT_RELAY = "wss://titi-relay-x3clqgvrdq-ew.a.run.app/v1/ws";
export const defaultSettings: Settings = {
  name: "",
  hue: 40,
  onboarded: false,
  relayUrl: DEFAULT_RELAY,
  volume: 1,
  pttKey: "",
  overlay: true,
  autostart: false,
  closeToTray: true,
  inputDevice: "",
  outputDevice: "",
  lan: true,
  relay: true,
  ble: true,
};

interface State {
  ready: boolean;
  running: boolean;
  platform: PlatformKind;
  voiceSupported: boolean;
  nodeId: string;
  settings: Settings;
  groups: GroupState[];
  activeGroup: string | null;
  peers: Record<string, Peer>;
  invites: Invite[];
  messages: Msg[];
  levelDbfs: number;
  muted: boolean;
  /** Global hotkey held (desktop) — mirrors the Talk button visually */
  pttKeyDown: boolean;
  /** Deep link delivered by the OS while running (desktop/tv) */
  pendingLink: string | null;
  /** Screen requested by the OS shell (jump list, toast, tray): group id or "join" */
  pendingRoute: string | null;
  /** Latest un-cancelled SOS from someone else, until acknowledged (TV shows it full screen). */
  sos: { group: string; groupName: string; fromName: string; latE7: number; lonE7: number; note: string } | null;
  toast: string | null;
  error: string | null;
  set: (p: Partial<State> | ((s: State) => Partial<State>)) => void;
  updateGroup: (id: string, f: (g: GroupState) => GroupState) => void;
}

export const useStore = create<State>((set) => ({
  ready: false,
  running: false,
  platform: "web",
  voiceSupported: true,
  nodeId: "",
  settings: defaultSettings,
  groups: [],
  activeGroup: null,
  peers: {},
  invites: [],
  messages: [],
  levelDbfs: -60,
  muted: false,
  pttKeyDown: false,
  pendingLink: null,
  pendingRoute: null,
  sos: null,
  toast: null,
  error: null,
  set: (p) => set(typeof p === "function" ? p : () => p),
  updateGroup: (id, f) => set((s) => ({ groups: s.groups.map((g) => (g.id === id ? f(g) : g)) })),
}));

export const activeGroup = (s: State) => s.groups.find((g) => g.id === s.activeGroup) ?? null;
