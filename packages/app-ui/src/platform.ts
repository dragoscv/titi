// The one seam between the shared UI and each platform. Web (wasm + WebCodecs +
// WSS), desktop (Tauri → native Rust engine) and TV hosts implement it; screens
// only ever call `host.*`.
import type { Settings } from "./store";

export type PlatformKind = "web" | "desktop" | "tv";

export interface Capabilities {
  /** Same-Wi-Fi UDP multicast (native hosts only). */
  lan: boolean;
  /** Bluetooth LE transport. */
  ble: boolean;
  /** System-wide hold-to-talk key (desktop). */
  globalPtt: boolean;
  /** Camera QR scanning. */
  qrScan: boolean;
  /** Microphone available / allowed at all. */
  mic: boolean;
  /** 10-foot UI: D-pad focus, no pointer. */
  tenFoot: boolean;
}

export interface AudioDevice { id: string; name: string; isDefault: boolean }

export interface TitiHost {
  readonly kind: PlatformKind;
  readonly caps: Capabilities;
  init(): Promise<void>;
  /** Must be called from a user gesture on web (AudioContext + mic). */
  start(): Promise<void>;
  saveSettings(p: Partial<Settings>): void;
  pttDown(): void;
  pttUp(): void;
  setMuted(m: boolean): void;
  createGroup(name: string): void;
  leaveGroup(id: string): void;
  setActiveGroup(id: string): void;
  setFullDuplex(id: string, on: boolean): void;
  invitePeer(group: string, node: string): void;
  acceptInvite(group: string, host: string): void;
  declineInvite(group: string, host: string): void;
  joinByCode(code: string): void;
  joinByLink(url: string): void;
  sendText(group: string, text: string): void;
  sendSos(group: string, cancelled: boolean): void;
  /** [code, seconds until it rotates] */
  currentCode(group: string): Promise<[string, number] | null>;
  deepLink(group: string): Promise<string | null>;
  parseCode(text: string): Promise<boolean>;
  /** Optional platform extras. */
  audioDevices?(): Promise<{ inputs: AudioDevice[]; outputs: AudioDevice[] }>;
  capturePttKey?(): Promise<string | null>;
  /** Stop the radio and exit the app completely (desktop, TV). Absent on web: close the tab. */
  quit?(): void;
}

let impl: TitiHost | null = null;

export function installHost(h: TitiHost) {
  impl = h;
}

/** Late-bound facade so screens can `import { host }` before the platform installs itself. */
export const host: TitiHost = new Proxy({} as TitiHost, {
  get(_, key: string | symbol) {
    if (!impl) throw new Error(`titi host not installed (reading ${String(key)})`);
    const v = (impl as unknown as Record<string | symbol, unknown>)[key];
    return typeof v === "function" ? (v as (...a: unknown[]) => unknown).bind(impl) : v;
  },
});
