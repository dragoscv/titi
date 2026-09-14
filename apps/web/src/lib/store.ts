"use client";
import { create } from "zustand";
import type { GroupJson, LinkClass, MsgBody, Profile } from "./types";

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

export interface Settings { name: string; hue: number; onboarded: boolean; relayUrl: string }
export const DEFAULT_RELAY = "wss://titi-relay-x3clqgvrdq-ew.a.run.app/v1/ws";

interface State {
  ready: boolean;
  running: boolean;
  nodeId: string;
  settings: Settings;
  groups: GroupState[];
  activeGroup: string | null;
  peers: Record<string, Peer>;
  invites: Invite[];
  messages: Msg[];
  levelDbfs: number;
  muted: boolean;
  toast: string | null;
  error: string | null;
  set: (p: Partial<State> | ((s: State) => Partial<State>)) => void;
  updateGroup: (id: string, f: (g: GroupState) => GroupState) => void;
}

export const useStore = create<State>((set) => ({
  ready: false,
  running: false,
  nodeId: "",
  settings: { name: "", hue: 40, onboarded: false, relayUrl: DEFAULT_RELAY },
  groups: [],
  activeGroup: null,
  peers: {},
  invites: [],
  messages: [],
  levelDbfs: -60,
  muted: false,
  toast: null,
  error: null,
  set: (p) => set(typeof p === "function" ? p : () => p),
  updateGroup: (id, f) => set((s) => ({ groups: s.groups.map((g) => (g.id === id ? f(g) : g)) })),
}));

export const activeGroup = (s: State) => s.groups.find((g) => g.id === s.activeGroup) ?? null;
